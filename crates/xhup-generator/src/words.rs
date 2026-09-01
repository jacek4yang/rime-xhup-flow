//! 规范高频词语数据(万象 / RIME-LMDG 提取子集)的解析与校验。
//!
//! 入库 TSV `data/words/wanxiang_base_words.tsv` 经 `include_str!` 嵌入,是
//! 固定高频词语层的唯一事实来源(来源、提取/选择/碰撞过滤规则与覆盖审计见该
//! 目录 README)。本模块不读写文件、不访问网络;TSV 损坏属于仓库不变量被破坏,
//! 解析时 panic 并给出精确行号。
//!
//! 词语数据按 `(词, 规范读音序列)` semantic entry 组织:同一词形的不同合法
//! 读音序列是独立条目,各有独立聚合分数。词码不在此层出现——编码推导与排名
//! 由 [`crate::word_codes`] 在生成期完成,不存在第二份手写映射。

use std::sync::OnceLock;

use xhup_core::{HanziReading, XhupHanzi};

/// 入库的规范词语 TSV(唯一事实来源;由仓库自带提取器可复现生成)。
const WORDS_TSV: &str = include_str!("../../../data/words/wanxiang_base_words.tsv");

/// 一条规范词语 semantic entry:`(词, 规范读音序列)` + 万象聚合分数。
///
/// 字段对 crate 内只读;词形零拷贝借用内嵌 TSV,逐字读音为规范类型化切片。
#[derive(Clone, Debug)]
pub(crate) struct CanonicalWordEntry {
    word: &'static str,
    readings: Box<[HanziReading]>,
    frequency_score: u64,
}

impl CanonicalWordEntry {
    /// 词形文本(2~4 个规范汉字)。
    pub(crate) fn word(&self) -> &'static str {
        self.word
    }

    /// 逐字规范读音(数量等于词长)。
    pub(crate) fn readings(&self) -> &[HanziReading] {
        &self.readings
    }

    /// 万象聚合分数(u64,严格为正)。
    pub(crate) fn frequency_score(&self) -> u64 {
        self.frequency_score
    }
}

/// 全部规范词语 semantic entry(进程内共享,解析一次)。
///
/// 顺序为 canonical 序列化顺序:词长升序 → 词 Unicode 升序 → 读音序列升序。
pub(crate) fn canonical_word_entries() -> &'static [CanonicalWordEntry] {
    static ENTRIES: OnceLock<Vec<CanonicalWordEntry>> = OnceLock::new();
    ENTRIES
        .get_or_init(|| parse_tsv(WORDS_TSV, "wanxiang_base_words.tsv"))
        .as_slice()
}

/// 解析内嵌 TSV:`#` 开头为注释头;数据行 `词<TAB>规范读音序列<TAB>分数`,
/// 读音序列以空格分隔。
///
/// 校验:恰好三个字段;词长 ∈ {2,3,4} 且读音数等于词长;逐字属于规范 8105
/// 清单、读音是该字规范读音、且可编码为 XHUP 输入音节;分数为正 u64;行按
/// (词长, 词, 读音序列) 严格升序(同时排除重复)。
fn parse_tsv(text: &'static str, name: &str) -> Vec<CanonicalWordEntry> {
    let mut entries: Vec<CanonicalWordEntry> = Vec::new();
    let mut previous_key: Option<(usize, &'static str, Vec<HanziReading>)> = None;
    for (index, line) in text.lines().enumerate() {
        let row_number = index + 1;
        if line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('\t');
        let (Some(word), Some(readings_field), Some(score_field), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            panic!("{name} 第 {row_number} 行应为三个 TAB 分隔字段: {line:?}");
        };

        let chars: Vec<XhupHanzi> = word
            .chars()
            .map(|ch| {
                XhupHanzi::try_from(ch).unwrap_or_else(|_| {
                    panic!("{name} 第 {row_number} 行汉字不在规范清单内: {ch:?}")
                })
            })
            .collect();
        assert!(
            (2..=4).contains(&chars.len()),
            "{name} 第 {row_number} 行词长应为 2~4 字: {word:?}"
        );

        let spellings: Vec<&str> = readings_field.split(' ').collect();
        assert!(
            spellings.len() == chars.len(),
            "{name} 第 {row_number} 行读音数应等于词长: {line:?}"
        );
        let mut readings: Vec<HanziReading> = Vec::with_capacity(chars.len());
        for (&hanzi, spelling) in chars.iter().zip(&spellings) {
            let reading = hanzi
                .readings()
                .iter()
                .copied()
                .find(|reading| reading.as_str() == *spelling)
                .unwrap_or_else(|| {
                    panic!("{name} 第 {row_number} 行读音不是该字的规范读音: {line:?}")
                });
            assert!(
                reading.to_input_syllable().is_some(),
                "{name} 第 {row_number} 行读音应可编码为 XHUP 输入音节: {line:?}"
            );
            readings.push(reading);
        }

        let score: u64 = score_field
            .parse()
            .unwrap_or_else(|_| panic!("{name} 第 {row_number} 行分数应为 u64: {score_field:?}"));
        assert!(score > 0, "{name} 第 {row_number} 行分数应为正数: {line:?}");

        let key = (chars.len(), word, readings.clone());
        if let Some(previous) = &previous_key {
            assert!(
                (previous.0, previous.1, &previous.2) < (key.0, key.1, &key.2),
                "{name} 第 {row_number} 行未按 (词长, 词, 读音序列) 严格升序(重复或乱序): {line:?}"
            );
        }
        previous_key = Some(key);

        entries.push(CanonicalWordEntry {
            word,
            readings: readings.into_boxed_slice(),
            frequency_score: score,
        });
    }
    assert!(!entries.is_empty(), "{name} 不应为空文件");
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_counts_match_committed_dataset() {
        let entries = canonical_word_entries();
        assert_eq!(entries.len(), 100_000);
        for (len, expected) in [(2, 50_000), (3, 30_000), (4, 20_000)] {
            assert_eq!(
                entries.iter().filter(|e| e.readings().len() == len).count(),
                expected,
                "{len} 字 semantic entries"
            );
        }
    }

    #[test]
    fn entries_are_strictly_ordered_and_unique() {
        let entries = canonical_word_entries();
        for pair in entries.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            assert!(
                (a.readings().len(), a.word(), a.readings())
                    < (b.readings().len(), b.word(), b.readings()),
                "canonical 顺序应严格递增(无重复)"
            );
        }
    }

    #[test]
    fn every_reading_is_canonical_and_encodable() {
        for entry in canonical_word_entries() {
            let chars: Vec<char> = entry.word().chars().collect();
            assert_eq!(chars.len(), entry.readings().len());
            for (&ch, &reading) in chars.iter().zip(entry.readings().iter()) {
                let hanzi = XhupHanzi::try_from(ch).unwrap();
                assert!(
                    hanzi.readings().contains(&reading),
                    "{} 的读音 {reading} 应为规范读音",
                    entry.word()
                );
                assert!(reading.to_input_syllable().is_some());
            }
            assert!(entry.frequency_score() > 0);
        }
    }
}
