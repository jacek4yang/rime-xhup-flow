//! optimizer v2 PRIMARY 词语简码层:canonical TSV 的解析与硬不变量校验。
//!
//! 入库 TSV `data/shortcuts/word_shortcuts_primary.tsv` 经 `include_str!`
//! 嵌入,是 optimizer v2 词语简码映射的主体层(与 FIXED_FIRST 层共同
//! 构成 v2 映射,替换 v1 的 ZERO_REGRESSION / 二码零冲突层;替换设计见
//! docs/optimizer-v2.md 与 data/shortcuts/README.md)。
//! 它由 xhup-analyzer 的 `export_v2` 从选定运行点的映射 dump 确定性导出
//! (provenance 见文件头注释),经 diff review 后入库;一旦发布即属于稳定
//! 的用户肌肉记忆兼容接口。
//!
//! 每条 `词<TAB>shortcut 码<TAB>rank<TAB>merged_rank`:rank 是该码
//! PRIMARY 子集内的稠密相对位次;merged_rank 是 baseline + v2 候选
//! 混排后的绝对候选位。菜单序由 merged_ranking 用唯一整数权重投影。
//!
//! 解析时的硬不变量(损坏即 panic,不修改数据迎合代码):
//!
//! - 词为 2~4 个规范汉字,且存在于 canonical 固定词层(alias,不替换完整码);
//! - shortcut 为纯小写 a-z,长度 2..=5 且短于该词完整码;
//! - 词全局唯一(本层内一词一码;与 FIXED_FIRST 层跨层一词一码,
//!   经双方 raw 扫描断言);
//! - 同一码内 rank 稠密 1..k,merged_rank 唯一且随 rank 严格递增;
//! - canonical 序列化顺序:shortcut 长度 → 码 → rank → 词。
//!
//! 本模块不读写文件、不访问网络。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use xhup_core::{KeySequence, XhupHanzi};

/// 入库的 PRIMARY 词语简码 TSV(唯一事实来源)。
const PRIMARY_SHORTCUTS_TSV: &str =
    include_str!("../../../data/shortcuts/word_shortcuts_primary.tsv");

/// PRIMARY 层的原始词集合(纯文本扫描,不经过本层校验管线)。
///
/// 跨层「一词一码」检查必须基于这种原始扫描,绝不能调用兄弟层的
/// OnceLock 校验管线(会形成循环初始化死锁)。
pub fn raw_words() -> BTreeSet<&'static str> {
    PRIMARY_SHORTCUTS_TSV
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| line.split('\t').next().expect("PRIMARY 数据行应有词字段"))
        .collect()
}

/// PRIMARY 层的原始 shortcut 码集合(纯文本扫描;语义约束同 [`raw_words`])。
pub fn raw_shortcut_codes() -> BTreeSet<KeySequence> {
    PRIMARY_SHORTCUTS_TSV
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let mut fields = line.split('\t');
            fields.next();
            fields
                .next()
                .expect("PRIMARY 数据行应有 shortcut 字段")
                .parse()
                .expect("PRIMARY shortcut 码应可解析")
        })
        .collect()
}

/// 初始化安全的排名原始视图:`(词, 码, PRIMARY rank, merged_rank)`。
///
/// merged_ranking 在单字/词语最终化过程中调用它,因此这里不得反向调用
/// canonical 词层。这个原始视图也严格要求四列;损坏的 production 数据不得
/// 通过返回空集降级。
pub(crate) fn raw_ranking_entries() -> Vec<(String, KeySequence, usize, usize)> {
    PRIMARY_SHORTCUTS_TSV
        .lines()
        .filter(|line| !line.starts_with('#'))
        .enumerate()
        .map(|(index, line)| {
            let fields: Vec<&str> = line.split('\t').collect();
            assert_eq!(
                fields.len(),
                4,
                "word_shortcuts_primary.tsv 第 {} 个数据行应有四列: {line:?}",
                index + 1
            );
            let word = fields.first().expect("PRIMARY 应有词字段").to_string();
            let code = fields
                .get(1)
                .expect("PRIMARY 应有 shortcut 字段")
                .parse()
                .expect("PRIMARY shortcut 应合法");
            let rank = fields
                .get(2)
                .expect("PRIMARY 应有 rank 字段")
                .parse()
                .expect("PRIMARY rank 应合法");
            let merged_rank = fields
                .get(3)
                .expect("PRIMARY 应有 merged_rank 字段")
                .parse()
                .expect("PRIMARY merged_rank 应合法");
            assert!(rank >= 1 && merged_rank >= rank, "PRIMARY raw rank 应合法");
            (word, code, rank, merged_rank)
        })
        .collect()
}

/// 一条 canonical PRIMARY 词语简码关系:一个词的一个 shortcut 别名及其
/// 码内候选位。
pub struct CanonicalPrimaryShortcutEntry {
    word: String,
    full_code: KeySequence,
    shortcut_code: KeySequence,
    rank: usize,
    merged_rank: usize,
}

impl CanonicalPrimaryShortcutEntry {
    /// 词语(2~4 个规范汉字)。
    pub fn word(&self) -> &str {
        &self.word
    }

    /// 该词 canonical 完整码(shortcut 只增加别名,不替换完整码)。
    pub fn full_code(&self) -> &KeySequence {
        &self.full_code
    }

    /// shortcut 码(纯小写 a-z,长度 2..=5 且短于该词完整码)。
    pub fn shortcut_code(&self) -> &KeySequence {
        &self.shortcut_code
    }

    /// 码内候选位(1 = 首选;同码 v2 条目按 rank 升序)。
    pub fn rank(&self) -> usize {
        self.rank
    }

    /// baseline + v2 混排后的绝对候选位(1 = 首选)。
    pub fn merged_rank(&self) -> usize {
        self.merged_rank
    }

    /// Rime 静态同码全组的唯一整数权重。
    pub fn rime_weight(&self) -> u32 {
        crate::merged_ranking::merged_weight(&self.shortcut_code, &self.word)
            .expect("PRIMARY 条目必须参与 merged ranking")
    }
}

/// 全部 canonical PRIMARY 词语简码关系(进程内共享,解析一次;
/// canonical 序列化顺序)。
pub fn canonical_primary_shortcut_entries() -> &'static [CanonicalPrimaryShortcutEntry] {
    static ENTRIES: OnceLock<Vec<CanonicalPrimaryShortcutEntry>> = OnceLock::new();
    ENTRIES
        .get_or_init(|| parse_tsv(PRIMARY_SHORTCUTS_TSV, "word_shortcuts_primary.tsv"))
        .as_slice()
}

/// 解析内嵌 TSV 并验证全部硬不变量。
fn parse_tsv(text: &'static str, name: &str) -> Vec<CanonicalPrimaryShortcutEntry> {
    // 固定词层的 词 → 完整码(成员资格与码长校验)。
    let word_full_codes: BTreeMap<String, KeySequence> = crate::canonical_word_code_entries()
        .into_iter()
        .map(|entry| (entry.word().to_string(), entry.code().clone()))
        .collect();
    // 跨层「一词一码」集合来自 FF 层的原始 TSV 文本扫描(防循环初始化)。
    let ff_words: BTreeSet<&str> = crate::fixed_first_shortcuts::raw_words();

    let mut entries: Vec<CanonicalPrimaryShortcutEntry> = Vec::new();
    let mut words: BTreeSet<&str> = BTreeSet::new();
    let mut pairs: BTreeSet<(String, String)> = BTreeSet::new();
    let mut ranks_by_code: BTreeMap<KeySequence, Vec<usize>> = BTreeMap::new();
    let mut merged_ranks_by_code: BTreeMap<KeySequence, Vec<(usize, usize)>> = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        let row_number = index + 1;
        if line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('\t');
        let (Some(word), Some(code_field), Some(rank_field), Some(merged_rank_field), None) = (
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
        ) else {
            panic!("{name} 第 {row_number} 行应为四个 TAB 分隔字段: {line:?}");
        };

        // 词:2~4 个规范汉字;在 canonical 固定词层;不得持有 FF 简码。
        let char_count = word.chars().count();
        assert!(
            (2..=4).contains(&char_count),
            "{name} 第 {row_number} 行词应为 2~4 字: {word:?}"
        );
        for ch in word.chars() {
            assert!(
                XhupHanzi::try_from(ch).is_ok(),
                "{name} 第 {row_number} 行含非规范汉字: {word:?} 的 {ch:?}"
            );
        }
        let full_code = word_full_codes
            .get(word)
            .unwrap_or_else(|| panic!("{name} 第 {row_number} 行词不在固定词层: {word:?}"));
        assert!(
            !ff_words.contains(word),
            "{name} 第 {row_number} 行词已持有 FIXED_FIRST 简码(跨层一词一码): {word:?}"
        );

        // shortcut:纯小写 a-z,长度 2..=5 且短于完整码。
        let shortcut_code: KeySequence = code_field
            .parse()
            .unwrap_or_else(|_| panic!("{name} 第 {row_number} 行 shortcut 码非法: {line:?}"));
        assert!(
            (2..=5).contains(&shortcut_code.len()) && shortcut_code.len() < full_code.len(),
            "{name} 第 {row_number} 行 shortcut 长度应在 2..=5 且短于完整码: {line:?}"
        );

        // rank:正整数。
        let rank: usize = rank_field
            .parse()
            .unwrap_or_else(|_| panic!("{name} 第 {row_number} 行 rank 应为正整数: {line:?}"));
        assert!(rank >= 1, "{name} 第 {row_number} 行 rank 应 ≥1: {line:?}");
        let merged_rank: usize = merged_rank_field.parse().unwrap_or_else(|_| {
            panic!("{name} 第 {row_number} 行 merged_rank 应为正整数: {line:?}")
        });
        assert!(
            merged_rank >= rank,
            "{name} 第 {row_number} 行 merged_rank 应 ≥ rank: {line:?}"
        );

        // 唯一性。
        assert!(
            words.insert(word),
            "{name} 第 {row_number} 行词重复: {word}"
        );
        assert!(
            pairs.insert((word.to_string(), code_field.to_string())),
            "{name} 第 {row_number} 行 (词, 码) 重复: {line:?}"
        );
        ranks_by_code
            .entry(shortcut_code.clone())
            .or_default()
            .push(rank);
        merged_ranks_by_code
            .entry(shortcut_code.clone())
            .or_default()
            .push((rank, merged_rank));

        entries.push(CanonicalPrimaryShortcutEntry {
            word: word.to_string(),
            full_code: full_code.clone(),
            shortcut_code,
            rank,
            merged_rank,
        });
    }
    assert!(!entries.is_empty(), "{name} 应包含数据行");

    // 码内 rank 稠密 1..k。
    for (code, mut ranks) in ranks_by_code {
        ranks.sort_unstable();
        let expected: Vec<usize> = (1..=ranks.len()).collect();
        assert_eq!(ranks, expected, "{name} 码 {code} 的 rank 应稠密 1..k");
    }
    for (code, mut ranks) in merged_ranks_by_code {
        ranks.sort_unstable();
        for pair in ranks.windows(2) {
            assert!(
                pair[0].1 < pair[1].1,
                "{name} 码 {code} 的 merged_rank 应随 rank 严格递增"
            );
        }
    }

    // canonical 序列化顺序:shortcut 长度 → 码 → rank → 词。
    for pair in entries.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let key = |e: &CanonicalPrimaryShortcutEntry| {
            (
                e.shortcut_code.len(),
                e.shortcut_code.clone(),
                e.rank,
                e.word.clone(),
            )
        };
        assert!(
            key(a) < key(b),
            "{name} 应严格按 canonical 序列化顺序: {:?} 之后是 {:?}",
            a.word,
            b.word
        );
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_parsed_and_nonempty() {
        let entries = canonical_primary_shortcut_entries();
        assert!(entries.len() > 60_000, "PRIMARY 层应有数万条");
        // 语义哨兵(选定点冻结):就是 jqu rank 1、我们 wm rank 1。
        let women = entries
            .iter()
            .find(|e| e.word() == "我们")
            .expect("我们 应有 PRIMARY 简码");
        assert_eq!(women.shortcut_code().to_string(), "wm");
        assert_eq!(women.rank(), 1);
        assert_eq!(women.merged_rank(), 1);
        let jiushi = entries
            .iter()
            .find(|e| e.word() == "就是")
            .expect("就是 应有 PRIMARY 简码");
        assert_eq!(jiushi.shortcut_code().to_string(), "jqu");
        assert_eq!(jiushi.rank(), 1);
        assert_eq!(jiushi.merged_rank(), 1);
    }

    #[test]
    fn every_entry_has_unique_word_and_dense_ranks() {
        // parse_tsv 的硬不变量已覆盖;此处为独立语义抽查(防止校验被误放松)。
        let entries = canonical_primary_shortcut_entries();
        let mut by_code: BTreeMap<&KeySequence, Vec<usize>> = BTreeMap::new();
        for entry in entries {
            by_code
                .entry(&entry.shortcut_code)
                .or_default()
                .push(entry.rank());
        }
        for (code, mut ranks) in by_code {
            ranks.sort_unstable();
            let expected: Vec<usize> = (1..=ranks.len()).collect();
            assert_eq!(ranks, expected, "{code} rank 应稠密");
        }
    }
}
