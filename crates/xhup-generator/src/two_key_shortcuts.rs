//! 二码零冲突词语简码层:canonical TSV 的解析与硬不变量校验。
//!
//! 入库 TSV `data/shortcuts/word_two_key_zero_regression.tsv` 经
//! `include_str!` 嵌入,是二码词语简码层的唯一事实来源。它由
//! xhup-analyzer 的 production selection policy
//! (`two-key-zero-regression-v1`,见 `xhup_analyzer::production_two_key`)
//! 从万象词语/频率证据确定性导出:**仅使用 2 键 exact-code 空间完全
//! 空闲的 `II` 理论候选**(candidate grammar
//! monotone-suffix-initials-v2;每 2 键码恰一词,整数 4/5 稳定票)。
//! 空码意味着新词是该码唯一的 exact 候选(rank 1),严格零冲突;
//! 占用码(char fanout > 0)不在本层。数据经 diff review 与 policy review
//! 后入库;一旦发布即属于稳定的用户肌肉记忆兼容接口(见
//! `data/shortcuts/README.md`)。
//!
//! 解析时的硬不变量(损坏即 panic,不修改数据迎合代码):
//!
//! - 每条 `词<TAB>完整码<TAB>shortcut 码<TAB>模式`,词为恰 2 个规范汉字;
//! - `(词, 完整码)` 必须存在于 canonical 固定词层(alias,不替换完整码);
//! - 完整码为 4 键;shortcut 为纯小写 a-z 2 键,且不等于完整码前缀外的
//!   任何规则 —— 模式必须恰为 `II`,机械投影(两字双拼首键)必须等于
//!   shortcut 码;
//! - shortcut 码在 baseline fixed exact-code 集合(一级简码 + 单字 2/3/4
//!   码 + 固定词 4/6/8 键)中必须**完全空闲**(fanout == 0)—— 二码
//!   零冲突语义由 generator 独立重验,不盲信 analyzer 输出;
//! - 词不得持有 ZERO_REGRESSION / FIXED_FIRST production 简码(一词
//!   最多一条简码);shortcut 码不得与两层的任何 production 码冲突;
//! - 词、shortcut 码、`(词, 完整码)` 各自唯一。
//!
//! 本模块不读写文件、不访问网络。

use std::collections::BTreeSet;
use std::sync::OnceLock;

use xhup_core::{KeySequence, XhupHanzi};

/// 入库的二码零冲突词语简码 TSV(唯一事实来源)。
const TWO_KEY_SHORTCUTS_TSV: &str =
    include_str!("../../../data/shortcuts/word_two_key_zero_regression.tsv");

/// 一条 canonical 二码词语简码关系:一个 2 字词的一个 2 键别名。
pub struct CanonicalTwoKeyShortcutEntry {
    word: String,
    full_code: KeySequence,
    shortcut_code: KeySequence,
    mode: String,
}

impl CanonicalTwoKeyShortcutEntry {
    /// 词语(恰 2 字)。
    pub fn word(&self) -> &str {
        &self.word
    }

    /// 完整码(4 键;保留可用,shortcut 不替换它)。
    pub fn full_code(&self) -> &KeySequence {
        &self.full_code
    }

    /// 2 键 II 简码(与 baseline 全部 exact-code 空闲码位)。
    pub fn shortcut_code(&self) -> &KeySequence {
        &self.shortcut_code
    }

    /// F/I 投影模式(恒 `II`)。
    pub fn mode(&self) -> &str {
        &self.mode
    }
}

/// 全部 canonical 二码词语简码关系(进程内共享,解析一次;canonical
/// 序列化顺序)。
pub fn canonical_two_key_shortcut_entries() -> &'static [CanonicalTwoKeyShortcutEntry] {
    static ENTRIES: OnceLock<Vec<CanonicalTwoKeyShortcutEntry>> = OnceLock::new();
    ENTRIES
        .get_or_init(|| parse_tsv(TWO_KEY_SHORTCUTS_TSV, "word_two_key_zero_regression.tsv"))
        .as_slice()
}

/// baseline fixed exact-code 集合(一级简码 + 单字 2/3/4 码 + 固定词
/// 4/6/8 键)。二码零冲突 shortcut 必须与其完全不相交。
fn baseline_fixed_codes() -> BTreeSet<KeySequence> {
    let mut codes = BTreeSet::new();
    for entry in crate::canonical_level1_shortcuts() {
        codes.insert(KeySequence::from_keys(&[entry.key()]).expect("一键非空"));
    }
    for entry in crate::canonical_char_code_entries() {
        codes.insert(entry.code().clone());
    }
    for entry in crate::canonical_word_code_entries() {
        codes.insert(entry.code().clone());
    }
    codes
}

/// 解析内嵌 TSV 并验证全部硬不变量。
fn parse_tsv(text: &'static str, name: &str) -> Vec<CanonicalTwoKeyShortcutEntry> {
    // 固定词层的 (词, 完整码) 成员资格与 baseline 码集合。
    let word_codes: BTreeSet<(String, String)> = crate::canonical_word_code_entries()
        .iter()
        .map(|entry| (entry.word().to_string(), entry.code().to_string()))
        .collect();
    let baseline_codes = baseline_fixed_codes();
    // 既有 ZR / FF 词与码集合(一词最多一码;码全量不相交)。
    let existing_words: BTreeSet<&str> = crate::canonical_word_shortcut_entries()
        .iter()
        .map(|entry| entry.word())
        .chain(
            crate::canonical_fixed_first_shortcut_entries()
                .iter()
                .map(|entry| entry.word()),
        )
        .collect();
    let existing_codes: BTreeSet<KeySequence> = crate::canonical_word_shortcut_entries()
        .iter()
        .map(|entry| entry.shortcut_code().clone())
        .chain(
            crate::canonical_fixed_first_shortcut_entries()
                .iter()
                .map(|entry| entry.shortcut_code().clone()),
        )
        .collect();

    let mut entries: Vec<CanonicalTwoKeyShortcutEntry> = Vec::new();
    let mut words: BTreeSet<&str> = BTreeSet::new();
    let mut codes: BTreeSet<KeySequence> = BTreeSet::new();
    let mut word_full_codes: BTreeSet<(String, String)> = BTreeSet::new();
    for (index, line) in text.lines().enumerate() {
        let row_number = index + 1;
        if line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('\t');
        let (Some(word), Some(full_field), Some(shortcut_field), Some(mode), None) = (
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
        ) else {
            panic!("{name} 第 {row_number} 行应为四个 TAB 分隔字段: {line:?}");
        };

        // 词:恰 2 个规范汉字;不得已有既有 production 简码。
        let char_count = word.chars().count();
        assert_eq!(
            char_count, 2,
            "{name} 第 {row_number} 行词应为恰 2 字: {word:?}"
        );
        for ch in word.chars() {
            assert!(
                XhupHanzi::try_from(ch).is_ok(),
                "{name} 第 {row_number} 行含非规范汉字: {word:?} 的 {ch:?}"
            );
        }
        assert!(
            !existing_words.contains(word),
            "{name} 第 {row_number} 行词已持有 ZR/FIXED_FIRST 简码: {word:?}"
        );

        // 完整码:4 键,且 (词, 完整码) 属于固定词层。
        let full_code: KeySequence = full_field
            .parse()
            .unwrap_or_else(|_| panic!("{name} 第 {row_number} 行完整码非法: {line:?}"));
        assert_eq!(
            full_code.len(),
            4,
            "{name} 第 {row_number} 行完整码应为 4 键: {line:?}"
        );
        assert!(
            word_codes.contains(&(word.to_string(), full_field.to_string())),
            "{name} 第 {row_number} 行 (词, 完整码) 不在固定词层: {line:?}"
        );

        // shortcut:纯小写 a-z 恰 2 键。
        let shortcut_code: KeySequence = shortcut_field
            .parse()
            .unwrap_or_else(|_| panic!("{name} 第 {row_number} 行 shortcut 码非法: {line:?}"));
        assert_eq!(
            shortcut_code.len(),
            2,
            "{name} 第 {row_number} 行 shortcut 应为 2 键: {line:?}"
        );

        // 模式:恒 II,机械投影 = 两字双拼首键。
        assert_eq!(
            mode, "II",
            "{name} 第 {row_number} 行模式必须为 II: {line:?}"
        );
        let full = full_code.as_slice();
        assert_eq!(
            shortcut_code.as_slice(),
            &[full[0], full[2]],
            "{name} 第 {row_number} 行 II 投影不一致: {line:?}"
        );

        // 二码零冲突语义独立重验:baseline 全部 exact-code 空间空闲,
        // 且不与既有 ZR/FF production 码冲突。
        assert!(
            !baseline_codes.contains(&shortcut_code),
            "{name} 第 {row_number} 行 shortcut 与 baseline fixed 码冲突(非空码): {line:?}"
        );
        assert!(
            !existing_codes.contains(&shortcut_code),
            "{name} 第 {row_number} 行 shortcut 与既有 production 码冲突: {line:?}"
        );

        // 唯一性。
        assert!(
            words.insert(word),
            "{name} 第 {row_number} 行词重复: {word}"
        );
        assert!(
            codes.insert(shortcut_code.clone()),
            "{name} 第 {row_number} 行码重复: {shortcut_code}"
        );
        assert!(
            word_full_codes.insert((word.to_string(), full_field.to_string())),
            "{name} 第 {row_number} 行 (词, 完整码) 重复: {line:?}"
        );

        entries.push(CanonicalTwoKeyShortcutEntry {
            word: word.to_string(),
            full_code,
            shortcut_code,
            mode: mode.to_string(),
        });
    }
    assert!(!entries.is_empty(), "{name} 应包含数据行");

    // canonical 序列化顺序:码 → 词 → 完整码(全部 2 键,长度无差异)。
    for pair in entries.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let key = |e: &CanonicalTwoKeyShortcutEntry| {
            (e.shortcut_code.clone(), e.word.clone(), e.full_code.clone())
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
        let entries = canonical_two_key_shortcut_entries();
        assert!(!entries.is_empty(), "二码零冲突层应有数据");
        // 层规模应显著小于 ZR 层(空码是稀缺空间)。
        assert!(
            entries.len() < crate::canonical_word_shortcut_entries().len(),
            "二码层应显著小于 ZERO_REGRESSION 层"
        );
    }
}
