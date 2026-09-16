//! 搜狗细胞词库生产构建分流(词库质量提升 PR-4)。
//!
//! 原始分片 `sogou_cell_*.tsv` 永不修改。生产组句/扩展词码只消费本模块
//! 过滤后的子集:
//!
//! - `protected`:保护白名单,任何清洗策略下保留;
//! - `target`:游戏/动漫/明星等小众分类,构建时排除;
//! - `llm_review` + `keep`:娱乐混合库中 LLM 判为日常词,保留;
//! - `llm_review` + `remove`:人名/作品名/生造词,构建时排除;
//! - 无标注(默认 keep):保留。
//!
//! 被排除的词仍可通过逐字音码 open composition 到达(编译器不决定语言
//! 边界)。过滤键为 `(词, 规范读音序列)`,与 tags/verdicts TSV 一致。

use std::collections::BTreeMap;
use std::sync::OnceLock;

use xhup_core::HanziReading;

use crate::words::{CanonicalWordEntry, canonical_sogou_word_entries};

const TAGS_TSV: &str = include_str!("../../../data/words/sogou/tags_compact.tsv");
const VERDICTS_TSV: &str = include_str!("../../../data/words/sogou/llm_review_verdicts.tsv");

/// 与 `tags_compact.tsv` 第三列一致的非默认标注。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SogouTag {
    Protected,
    Target,
    LlmReview,
}

/// 与 `llm_review_verdicts.tsv` 第三列一致的精判。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LlmVerdict {
    Keep,
    Remove,
}

struct SogouFilterIndex {
    /// 词 → (读音序列 → 标注)。默认 keep 不入表。
    tags: BTreeMap<&'static str, BTreeMap<&'static str, SogouTag>>,
    /// 词 → (读音序列 → 判定)。仅覆盖 `llm_review` 行。
    verdicts: BTreeMap<&'static str, BTreeMap<&'static str, LlmVerdict>>,
}

fn index() -> &'static SogouFilterIndex {
    static INDEX: OnceLock<SogouFilterIndex> = OnceLock::new();
    INDEX.get_or_init(|| {
        let tags = parse_tags(TAGS_TSV, "tags_compact.tsv");
        let verdicts = parse_verdicts(VERDICTS_TSV, "llm_review_verdicts.tsv");
        let mut llm_review_rows = 0usize;
        for (word, readings) in &tags {
            for (reading, tag) in readings {
                if *tag != SogouTag::LlmReview {
                    continue;
                }
                llm_review_rows += 1;
                assert!(
                    verdicts
                        .get(word)
                        .and_then(|inner| inner.get(reading))
                        .is_some(),
                    "llm_review 缺少判定: {word}\t{reading}"
                );
            }
        }
        let mut verdict_rows = 0usize;
        for (word, readings) in &verdicts {
            for reading in readings.keys() {
                verdict_rows += 1;
                assert_eq!(
                    tags.get(word).and_then(|inner| inner.get(*reading)),
                    Some(&SogouTag::LlmReview),
                    "判定必须对应 llm_review 标注: {word}\t{reading}"
                );
            }
        }
        assert_eq!(
            llm_review_rows, verdict_rows,
            "llm_review 标注行数必须与判定行数一致"
        );
        SogouFilterIndex { tags, verdicts }
    })
}

fn parse_tags(
    text: &'static str,
    name: &str,
) -> BTreeMap<&'static str, BTreeMap<&'static str, SogouTag>> {
    let mut tags: BTreeMap<&'static str, BTreeMap<&'static str, SogouTag>> = BTreeMap::new();
    for (offset, line) in text.lines().enumerate() {
        let row_number = offset + 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('\t');
        let (Some(word), Some(reading), Some(tag_field), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            panic!("{name} 第 {row_number} 行应为三个 TAB 分隔字段: {line:?}");
        };
        let tag = match tag_field {
            "protected" => SogouTag::Protected,
            "target" => SogouTag::Target,
            "llm_review" => SogouTag::LlmReview,
            other => panic!("{name} 第 {row_number} 行未知标注: {other:?}"),
        };
        let previous = tags.entry(word).or_default().insert(reading, tag);
        if let Some(previous) = previous {
            assert_eq!(
                previous, tag,
                "{name} 第 {row_number} 行 (词, 读音) 标注冲突: {word}\t{reading}"
            );
        }
    }
    tags
}

fn parse_verdicts(
    text: &'static str,
    name: &str,
) -> BTreeMap<&'static str, BTreeMap<&'static str, LlmVerdict>> {
    let mut verdicts: BTreeMap<&'static str, BTreeMap<&'static str, LlmVerdict>> = BTreeMap::new();
    for (offset, line) in text.lines().enumerate() {
        let row_number = offset + 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('\t');
        let (Some(word), Some(reading), Some(verdict_field), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            panic!("{name} 第 {row_number} 行应为三个 TAB 分隔字段: {line:?}");
        };
        let verdict = match verdict_field {
            "keep" => LlmVerdict::Keep,
            "remove" => LlmVerdict::Remove,
            other => panic!("{name} 第 {row_number} 行未知判定: {other:?}"),
        };
        let previous = verdicts.entry(word).or_default().insert(reading, verdict);
        if let Some(previous) = previous {
            assert_eq!(
                previous, verdict,
                "{name} 第 {row_number} 行 (词, 读音) 判定冲突: {word}\t{reading}"
            );
        }
    }
    verdicts
}

fn readings_field(readings: &[HanziReading]) -> String {
    let mut field = String::with_capacity(readings.len() * 4);
    for (index, reading) in readings.iter().enumerate() {
        if index > 0 {
            field.push(' ');
        }
        field.push_str(reading.as_str());
    }
    field
}

/// 一条搜狗 semantic entry 是否进入生产构建。
pub(crate) fn keep_sogou_production_entry(word: &str, readings: &[HanziReading]) -> bool {
    let reading = readings_field(readings);
    let tag = index()
        .tags
        .get(word)
        .and_then(|inner| inner.get(reading.as_str()));
    match tag {
        None | Some(SogouTag::Protected) => true,
        Some(SogouTag::Target) => false,
        Some(SogouTag::LlmReview) => {
            match index()
                .verdicts
                .get(word)
                .and_then(|inner| inner.get(reading.as_str()))
            {
                Some(LlmVerdict::Keep) => true,
                Some(LlmVerdict::Remove) | None => false,
            }
        }
    }
}

/// 生产构建使用的搜狗 semantic entry(原始分片顺序,已排除 target /
/// llm_review-remove)。进程内共享,过滤一次。
pub(crate) fn production_sogou_word_entries() -> &'static [&'static CanonicalWordEntry] {
    static ENTRIES: OnceLock<Vec<&'static CanonicalWordEntry>> = OnceLock::new();
    ENTRIES
        .get_or_init(|| {
            canonical_sogou_word_entries()
                .iter()
                .filter(|entry| keep_sogou_production_entry(entry.word(), entry.readings()))
                .collect()
        })
        .as_slice()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_parser_accepts_comments_and_rejects_unknown_labels() {
        let parsed = parse_tags(
            "# comment\n\n词\tdu yin\tprotected\n另一\tling yi\ttarget\n",
            "fixture",
        );
        assert_eq!(
            parsed.get("词").and_then(|inner| inner.get("du yin")),
            Some(&SogouTag::Protected)
        );
        assert_eq!(
            parsed.get("另一").and_then(|inner| inner.get("ling yi")),
            Some(&SogouTag::Target)
        );
    }

    #[test]
    #[should_panic(expected = "未知标注")]
    fn tag_parser_rejects_keep_rows() {
        let _ = parse_tags("词\tdu yin\tkeep\n", "fixture");
    }

    #[test]
    #[should_panic(expected = "未知判定")]
    fn verdict_parser_rejects_unknown_label() {
        let _ = parse_verdicts("词\tdu yin\tskip\n", "fixture");
    }

    #[test]
    fn committed_tag_and_verdict_counts_match_readme() {
        let idx = index();
        let mut protected = 0usize;
        let mut target = 0usize;
        let mut llm_review = 0usize;
        for readings in idx.tags.values() {
            for tag in readings.values() {
                match tag {
                    SogouTag::Protected => protected += 1,
                    SogouTag::Target => target += 1,
                    SogouTag::LlmReview => llm_review += 1,
                }
            }
        }
        assert_eq!(protected, 276_922);
        assert_eq!(target, 273_340);
        assert_eq!(llm_review, 55_684);

        let mut keep = 0usize;
        let mut remove = 0usize;
        for readings in idx.verdicts.values() {
            for verdict in readings.values() {
                match verdict {
                    LlmVerdict::Keep => keep += 1,
                    LlmVerdict::Remove => remove += 1,
                }
            }
        }
        assert_eq!(keep, 9_279);
        assert_eq!(remove, 46_405);
        assert_eq!(keep + remove, llm_review);
    }

    #[test]
    fn production_subset_excludes_target_and_llm_remove_only() {
        let raw = canonical_sogou_word_entries();
        let production = production_sogou_word_entries();
        assert_eq!(raw.len(), 2_082_859);
        assert_eq!(production.len(), 1_763_114);
        assert_eq!(raw.len() - production.len(), 273_340 + 46_405);

        let mut raw_index = 0usize;
        for entry in production {
            while raw_index < raw.len() && !std::ptr::eq(&raw[raw_index], *entry) {
                raw_index += 1;
            }
            assert!(raw_index < raw.len(), "生产子集必须保持原始分片相对顺序");
        }

        assert!(production.iter().any(|entry| {
            entry.word() == "一辑" && readings_field(entry.readings()) == "yi ji"
        }));
        assert!(production.iter().any(|entry| {
            entry.word() == "一丑" && readings_field(entry.readings()) == "yi chou"
        }));
        assert!(production.iter().any(|entry| {
            entry.word() == "一一" && readings_field(entry.readings()) == "yi yi"
        }));
        assert!(production.iter().all(|entry| {
            !(entry.word() == "一仇" && readings_field(entry.readings()) == "yi chou")
        }));
        assert!(production.iter().all(|entry| {
            !(entry.word() == "丁于" && readings_field(entry.readings()) == "ding yu")
        }));
    }
}
