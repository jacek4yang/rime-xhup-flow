//! 高稳健零冲突词语简码层:canonical TSV 的解析与硬不变量校验。
//!
//! 入库 TSV `data/shortcuts/word_zero_regression.tsv` 经 `include_str!` 嵌入,
//! 是这层词语简码的唯一事实来源。它由 xhup-analyzer 的 production selection
//! policy(`zero-regression-high-v1`,见 `xhup_analyzer::production`)从万象
//! 词语/频率证据确定性导出,经 diff review 与 policy review 后入库;一旦发布
//! 即属于稳定的用户肌肉记忆兼容接口,不随 analyzer 算法演进静默重生成
//! (数据性质与 provenance 见 `data/shortcuts/README.md`)。
//!
//! 解析时的硬不变量(损坏即 panic,不修改数据迎合代码):
//!
//! - 每条 `词<TAB>完整码<TAB>shortcut 码<TAB>模式`,词为 2~4 个规范汉字;
//! - `(词, 完整码)` 必须存在于 canonical 固定词层(alias,不替换完整码);
//! - shortcut 为纯小写 a-z,长度 ≥ 3 且小于完整码;
//! - 模式只含 F/I,字符数等于字数,且由 `完整码 + 模式` 机械投影必须恰好
//!   等于 shortcut 码(F = 完整两键,I = 双拼首键,无其它编码规则);
//! - 词、shortcut 码、`(词, 完整码)` 各自唯一;
//! - shortcut 码不在 baseline fixed exact-code 集合(一级简码 + 单字 2/3/4
//!   码 + 固定词 4/6/8 键)内 —— ZERO_REGRESSION 安全性由 generator 独立
//!   重验,不盲信 analyzer 输出。
//!
//! 本模块不读写文件、不访问网络。

use std::collections::BTreeSet;
use std::sync::OnceLock;

use xhup_core::{KeySequence, XhupHanzi};

/// 入库的词语简码 TSV(唯一事实来源)。
const WORD_SHORTCUTS_TSV: &str = include_str!("../../../data/shortcuts/word_zero_regression.tsv");

/// 一条 canonical 词语简码关系:一个词的一个 shortcut 别名。
pub struct CanonicalWordShortcutEntry {
    word: String,
    full_code: KeySequence,
    shortcut_code: KeySequence,
    mode: String,
}

impl CanonicalWordShortcutEntry {
    /// 词语(2~4 个规范汉字)。
    pub fn word(&self) -> &str {
        &self.word
    }

    /// 完整码(保留可用;shortcut 不替换它)。
    pub fn full_code(&self) -> &KeySequence {
        &self.full_code
    }

    /// shortcut 码(长度 ≥ 3 且小于完整码)。
    pub fn shortcut_code(&self) -> &KeySequence {
        &self.shortcut_code
    }

    /// F/I 投影模式(如 `FI`)。
    pub fn mode(&self) -> &str {
        &self.mode
    }
}

/// 全部 canonical 词语简码关系(进程内共享,解析一次;canonical 序列化顺序)。
pub fn canonical_word_shortcut_entries() -> &'static [CanonicalWordShortcutEntry] {
    static ENTRIES: OnceLock<Vec<CanonicalWordShortcutEntry>> = OnceLock::new();
    ENTRIES
        .get_or_init(|| parse_tsv(WORD_SHORTCUTS_TSV, "word_zero_regression.tsv"))
        .as_slice()
}

/// 极小 F/I 投影验证器:完整码每两键一个字,F 取两键、I 取首键。
///
/// 不涉及形码、读音查询或任何其它编码规则;仅用于机械校验 TSV 中
/// `完整码 + 模式 → shortcut` 的一致性。
fn project_shortcut(full_code: &KeySequence, mode: &str) -> Option<KeySequence> {
    let keys = full_code.as_slice();
    let (chunks, _) = keys.as_chunks::<2>();
    let mut shortcut = Vec::new();
    if chunks.len() != mode.chars().count() {
        return None;
    }
    for (chunk, mode_char) in chunks.iter().zip(mode.chars()) {
        match mode_char {
            'F' => shortcut.extend_from_slice(chunk),
            'I' => shortcut.push(chunk[0]),
            _ => return None,
        }
    }
    KeySequence::from_keys(&shortcut).ok()
}

/// baseline fixed exact-code 集合:一级简码 1 键 + 单字 2/3/4 码 + 固定词
/// 4/6/8 键。production shortcut 必须与其完全不相交。
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
fn parse_tsv(text: &'static str, name: &str) -> Vec<CanonicalWordShortcutEntry> {
    // 固定词层的 (词, 完整码) 成员资格与 baseline 码集合。
    let word_codes: BTreeSet<(String, String)> = crate::canonical_word_code_entries()
        .iter()
        .map(|entry| (entry.word().to_string(), entry.code().to_string()))
        .collect();
    let baseline_codes = baseline_fixed_codes();

    let mut entries: Vec<CanonicalWordShortcutEntry> = Vec::new();
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

        // 词:2~4 个规范汉字(逐字经 XhupHanzi 语义验证)。
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

        // 完整码:可解析,长度为字数两倍,且 (词, 完整码) 属于固定词层。
        let full_code: KeySequence = full_field
            .parse()
            .unwrap_or_else(|_| panic!("{name} 第 {row_number} 行完整码非法: {line:?}"));
        assert_eq!(
            full_code.len(),
            char_count * 2,
            "{name} 第 {row_number} 行完整码长度应为字数两倍: {line:?}"
        );
        assert!(
            word_codes.contains(&(word.to_string(), full_field.to_string())),
            "{name} 第 {row_number} 行 (词, 完整码) 不在固定词层: {line:?}"
        );

        // shortcut:纯小写 a-z,长度 ≥ 3 且小于完整码。
        let shortcut_code: KeySequence = shortcut_field
            .parse()
            .unwrap_or_else(|_| panic!("{name} 第 {row_number} 行 shortcut 码非法: {line:?}"));
        assert!(
            shortcut_code.len() >= 3 && shortcut_code.len() < full_code.len(),
            "{name} 第 {row_number} 行 shortcut 长度应在 [3, 完整码) 内: {line:?}"
        );

        // 模式:只含 F/I,字数匹配,机械投影必须等于 shortcut。
        assert!(
            mode.chars().count() == char_count && mode.chars().all(|c| matches!(c, 'F' | 'I')),
            "{name} 第 {row_number} 行模式应为恰等于字数的 F/I 串: {line:?}"
        );
        let projected = project_shortcut(&full_code, mode)
            .unwrap_or_else(|| panic!("{name} 第 {row_number} 行模式无法投影: {line:?}"));
        assert_eq!(
            projected, shortcut_code,
            "{name} 第 {row_number} 行 shortcut 与 完整码+模式 投影不一致: {line:?}"
        );

        // ZERO_REGRESSION 安全性独立重验。
        assert!(
            !baseline_codes.contains(&shortcut_code),
            "{name} 第 {row_number} 行 shortcut 与 baseline fixed 码冲突: {line:?}"
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

        entries.push(CanonicalWordShortcutEntry {
            word: word.to_string(),
            full_code,
            shortcut_code,
            mode: mode.to_string(),
        });
    }
    assert!(!entries.is_empty(), "{name} 应包含数据行");

    // canonical 序列化顺序:shortcut 长度 → 码 → 词 → 完整码 → 模式。
    for pair in entries.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let key = |e: &CanonicalWordShortcutEntry| {
            (
                e.shortcut_code.len(),
                e.shortcut_code.clone(),
                e.word.clone(),
                e.full_code.clone(),
                e.mode.clone(),
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

    /// 投影验证器的 frozen test vectors(「时间」不进入 production TSV,
    /// 但非常适合锁 projection 语义)。
    #[test]
    fn projection_matches_frozen_vectors() {
        let full: KeySequence = "uijm".parse().unwrap();
        let uij: KeySequence = "uij".parse().unwrap();
        let ujm: KeySequence = "ujm".parse().unwrap();
        assert_eq!(project_shortcut(&full, "FI"), Some(uij));
        assert_eq!(project_shortcut(&full, "IF"), Some(ujm));
        // 等于完整码的排除由 TSV 长度校验负责,投影函数本身不拦截
        assert_eq!(project_shortcut(&full, "FF").unwrap().to_string(), "uijm");
        assert!(project_shortcut(&full, "II").is_some()); // 投影本身合法(长度由 TSV 校验拦截)
        assert_eq!(project_shortcut(&full, "FI").unwrap().len(), 3);
    }

    #[test]
    fn entries_are_parsed_and_nonempty() {
        let entries = canonical_word_shortcut_entries();
        assert!(entries.len() > 40_000, "production 简码层应有数万条");
    }
}
