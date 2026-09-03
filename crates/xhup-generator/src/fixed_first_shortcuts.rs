//! 高稳健 FIXED_FIRST 词语简码层:canonical TSV 的解析与硬不变量校验。
//!
//! 入库 TSV `data/shortcuts/word_fixed_first.tsv` 经 `include_str!` 嵌入,
//! 是第二层词语简码的唯一事实来源。它由 xhup-analyzer 的 production
//! selection policy(`fixed-first-high-v1`,见
//! `xhup_analyzer::production_fixed_first`)在 ZERO_REGRESSION 层之上的
//! incremental universe 中确定性导出:与 baseline fixed exact code 重码、
//! 但频率/输入成本分析表明仍值得使用的高稳健候选,作为新增候选追加到既有
//! 固定候选之后(名次 = baseline fanout + 1,既有次序绝对不变)。数据经
//! diff review 与 policy review 后入库;一旦发布即属于稳定的用户肌肉记忆
//! 兼容接口,不随 analyzer 算法演进静默重生成(见 `data/shortcuts/README.md`)。
//!
//! 解析时的硬不变量(损坏即 panic,不修改数据迎合代码):
//!
//! - 每条 `词<TAB>完整码<TAB>shortcut 码<TAB>模式`,词为 2~4 个规范汉字;
//! - `(词, 完整码)` 必须存在于 canonical 固定词层(alias,不替换完整码);
//! - shortcut 为纯小写 a-z,长度 ≥ 3 且小于完整码;
//! - 模式只含 F/I,字符数等于字数,且 `完整码 + 模式` 机械投影必须恰好
//!   等于 shortcut 码(F = 完整两键,I = 双拼首键,无其它编码规则),
//!   且模式必须是单调后缀缩写 `F* I*`(candidate grammar
//!   monotone-suffix-initials-v2;一旦 I 出现,后续不得再 F —— 与
//!   analyzer 的 `CandidateGrammar::MonotoneSuffixInitialsV2` 是同一
//!   不变式的独立实现,generator 不依赖 analyzer);
//! - 词不得持有 ZERO_REGRESSION production 简码(一词最多一条简码);
//! - shortcut 码不得与 ZERO_REGRESSION production 码冲突(与 ZR 层全量
//!   不相交);
//! - shortcut 码必须命中 baseline fixed exact-code 集合(一级简码 + 单字
//!   2/3/4 码 + 固定词 4/6/8 键)—— FIXED_FIRST 语义本身就是「与固定候选
//!   重码」,由 generator 独立重验,不盲信 analyzer 输出;
//! - 同一 `(词, shortcut 码)` 不得已是 baseline exact 关系(否则第二
//!   translator 会制造重复候选);
//! - 词、shortcut 码、`(词, 完整码)` 各自唯一。
//!
//! 本模块不读写文件、不访问网络。

use std::collections::BTreeSet;
use std::sync::OnceLock;

use xhup_core::{KeySequence, XhupHanzi};

/// 入库的 FIXED_FIRST 词语简码 TSV(唯一事实来源)。
const FIXED_FIRST_SHORTCUTS_TSV: &str =
    include_str!("../../../data/shortcuts/word_fixed_first.tsv");

/// 一条 canonical FIXED_FIRST 词语简码关系:一个词的一个 shortcut 别名。
pub struct CanonicalFixedFirstShortcutEntry {
    word: String,
    full_code: KeySequence,
    shortcut_code: KeySequence,
    mode: String,
}

impl CanonicalFixedFirstShortcutEntry {
    /// 词语(2~4 个规范汉字)。
    pub fn word(&self) -> &str {
        &self.word
    }

    /// 完整码(保留可用;shortcut 不替换它)。
    pub fn full_code(&self) -> &KeySequence {
        &self.full_code
    }

    /// shortcut 码(长度 ≥ 3 且小于完整码,与 baseline fixed 码重码)。
    pub fn shortcut_code(&self) -> &KeySequence {
        &self.shortcut_code
    }

    /// F/I 投影模式(如 `FI`)。
    pub fn mode(&self) -> &str {
        &self.mode
    }
}

/// 全部 canonical FIXED_FIRST 词语简码关系(进程内共享,解析一次;
/// canonical 序列化顺序)。
pub fn canonical_fixed_first_shortcut_entries() -> &'static [CanonicalFixedFirstShortcutEntry] {
    static ENTRIES: OnceLock<Vec<CanonicalFixedFirstShortcutEntry>> = OnceLock::new();
    ENTRIES
        .get_or_init(|| parse_tsv(FIXED_FIRST_SHORTCUTS_TSV, "word_fixed_first.tsv"))
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

/// 模式是否为单调后缀缩写 `F* I*`(candidate grammar
/// monotone-suffix-initials-v2):只含 F/I,一旦 I 出现后续不得再 F。
///
/// 与 analyzer 侧 `CandidateGrammar::MonotoneSuffixInitialsV2::accepts` 是
/// 同一小不变式的独立实现(generator 不依赖 analyzer);本层 TSV 中的
/// 全部模式必须满足它。
fn is_monotone_suffix_mode(mode: &str) -> bool {
    let mut seen_initial = false;
    for c in mode.chars() {
        match c {
            'F' => {
                if seen_initial {
                    return false;
                }
            }
            'I' => seen_initial = true,
            _ => return false,
        }
    }
    seen_initial
}

/// baseline fixed exact-code 候选组:码 → 候选文本集合。FIXED_FIRST
/// shortcut 必须命中它(fanout > 0),且同码不得已有同名候选。
fn baseline_fixed_groups() -> std::collections::BTreeMap<KeySequence, BTreeSet<String>> {
    let mut groups: std::collections::BTreeMap<KeySequence, BTreeSet<String>> =
        std::collections::BTreeMap::new();
    for entry in crate::canonical_level1_shortcuts() {
        groups
            .entry(KeySequence::from_keys(&[entry.key()]).expect("一键非空"))
            .or_default()
            .insert(entry.hanzi().as_char().to_string());
    }
    for entry in crate::canonical_char_code_entries() {
        groups
            .entry(entry.code().clone())
            .or_default()
            .insert(entry.hanzi().as_char().to_string());
    }
    for entry in crate::canonical_word_code_entries() {
        groups
            .entry(entry.code().clone())
            .or_default()
            .insert(entry.word().to_string());
    }
    groups
}

/// 解析内嵌 TSV 并验证全部硬不变量。
fn parse_tsv(text: &'static str, name: &str) -> Vec<CanonicalFixedFirstShortcutEntry> {
    // 固定词层的 (词, 完整码) 成员资格、baseline 组与 ZR 词/码集合。
    let word_codes: BTreeSet<(String, String)> = crate::canonical_word_code_entries()
        .iter()
        .map(|entry| (entry.word().to_string(), entry.code().to_string()))
        .collect();
    let baseline_groups = baseline_fixed_groups();
    let zr_words: BTreeSet<&str> = crate::canonical_word_shortcut_entries()
        .iter()
        .map(|entry| entry.word())
        .collect();
    let zr_codes: BTreeSet<KeySequence> = crate::canonical_word_shortcut_entries()
        .iter()
        .map(|entry| entry.shortcut_code().clone())
        .collect();

    let mut entries: Vec<CanonicalFixedFirstShortcutEntry> = Vec::new();
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

        // 词:2~4 个规范汉字(逐字经 XhupHanzi 语义验证);不得已有 ZR 简码。
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
        assert!(
            !zr_words.contains(word),
            "{name} 第 {row_number} 行词已持有 ZERO_REGRESSION 简码: {word:?}"
        );

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

        // shortcut:纯小写 a-z,长度 ≥ 3 且小于完整码;不与 ZR 码冲突。
        let shortcut_code: KeySequence = shortcut_field
            .parse()
            .unwrap_or_else(|_| panic!("{name} 第 {row_number} 行 shortcut 码非法: {line:?}"));
        assert!(
            shortcut_code.len() >= 3 && shortcut_code.len() < full_code.len(),
            "{name} 第 {row_number} 行 shortcut 长度应在 [3, 完整码) 内: {line:?}"
        );
        assert!(
            !zr_codes.contains(&shortcut_code),
            "{name} 第 {row_number} 行 shortcut 与 ZERO_REGRESSION 码冲突: {line:?}"
        );

        // FIXED_FIRST 语义独立重验:必须与 baseline fixed 码重码,
        // 且同码不得已有同名候选(否则第二 translator 产生重复候选)。
        let baseline_texts = baseline_groups.get(&shortcut_code).unwrap_or_else(|| {
            panic!("{name} 第 {row_number} 行 shortcut 未命中 baseline fixed 码: {line:?}")
        });
        assert!(
            !baseline_texts.contains(word),
            "{name} 第 {row_number} 行 (词, shortcut) 已是 baseline exact 关系: {line:?}"
        );

        // 模式:只含 F/I,字数匹配,机械投影必须等于 shortcut,
        // 且必须是单调后缀缩写 F* I*(monotone-suffix-initials-v2)。
        assert!(
            mode.chars().count() == char_count && mode.chars().all(|c| matches!(c, 'F' | 'I')),
            "{name} 第 {row_number} 行模式应为恰等于字数的 F/I 串: {line:?}"
        );
        assert!(
            is_monotone_suffix_mode(mode),
            "{name} 第 {row_number} 行模式 {mode} 非单调后缀缩写(F* I*): {line:?}"
        );
        let projected = project_shortcut(&full_code, mode)
            .unwrap_or_else(|| panic!("{name} 第 {row_number} 行模式无法投影: {line:?}"));
        assert_eq!(
            projected, shortcut_code,
            "{name} 第 {row_number} 行 shortcut 与 完整码+模式 投影不一致: {line:?}"
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

        entries.push(CanonicalFixedFirstShortcutEntry {
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
        let key = |e: &CanonicalFixedFirstShortcutEntry| {
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

    /// 单调后缀缩写不变量(candidates 语法 monotone-suffix-initials-v2)。
    #[test]
    fn monotone_suffix_mode_vectors() {
        for valid in [
            "FI", "II", "FFI", "FII", "III", "FFFI", "FFII", "FIII", "IIII",
        ] {
            assert!(is_monotone_suffix_mode(valid), "应接受 {valid}");
        }
        // all-F 等于完整码,不是 shortcut;I 后再 F 非单调;其它字符非法。
        for invalid in [
            "", "F", "FF", "FFF", "FFFF", "IF", "IFI", "IFF", "IIF", "FIF", "IIIF", "IFII", "IIFI",
            "FX", "IFX",
        ] {
            assert!(!is_monotone_suffix_mode(invalid), "应拒绝 {invalid:?}");
        }
    }

    #[test]
    fn entries_are_parsed_and_nonempty() {
        let entries = canonical_fixed_first_shortcut_entries();
        assert!(!entries.is_empty(), "production FIXED_FIRST 层应有数据");
        assert!(
            entries.len() < crate::canonical_word_shortcut_entries().len(),
            "FIXED_FIRST 层应显著小于 ZERO_REGRESSION 层"
        );
    }
}
