//! optimizer v2 FIXED_FIRST 词语简码层:canonical TSV 解析与硬不变量校验。
//!
//! 入库 TSV `data/shortcuts/word_fixed_first.tsv` 经 `include_str!` 嵌入,
//! 是 v2 映射中「baseline 占用码上绝对 rank 1 且满足单调 F/I 格式」
//! 的子集;其余 v2 映射位于 PRIMARY。两层由同一 selected dump 确定性
//! 导出并共同构成唯一 production mapping。
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
//! - 词不得同时出现在 PRIMARY(跨层一词一码);
//! - shortcut 码必须命中 baseline fixed exact-code 集合(一级简码 + 单字
//!   2/3/4 码 + 固定词 4/6/8 键)—— FIXED_FIRST 语义本身就是「与固定候选
//!   重码」,由 generator 独立重验,不盲信 analyzer 输出;
//! - 同一 `(词, shortcut 码)` 不得已是 baseline exact 关系;
//! - 词、shortcut 码、`(词, 完整码)` 各自唯一。
//!
//! 本模块不读写文件、不访问网络。

use std::collections::BTreeSet;
use std::sync::OnceLock;

use xhup_core::{KeySequence, XhupHanzi};

/// 入库的 FIXED_FIRST 词语简码 TSV(唯一事实来源)。
const FIXED_FIRST_SHORTCUTS_TSV: &str =
    include_str!("../../../data/shortcuts/word_fixed_first.tsv");

/// v1 selector 的冻结 FIXED_FIRST fixture,仅供 analyzer 历史研究重放。
const LEGACY_V1_FIXED_FIRST_SHORTCUTS_TSV: &str =
    include_str!("../../../data/shortcuts/legacy/word_fixed_first_v1.tsv");

/// FF 层的原始词集合(纯文本扫描,不经过本层校验管线)。
///
/// 跨层「一词一码」检查必须基于这种原始扫描,绝不能调用兄弟层的
/// OnceLock 校验管线(会形成循环初始化死锁)。
pub fn raw_words() -> BTreeSet<&'static str> {
    FIXED_FIRST_SHORTCUTS_TSV
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| line.split('\t').next().expect("FF 数据行应有词字段"))
        .collect()
}

/// FF 层的原始 shortcut 码集合(纯文本扫描;语义约束同 [`raw_words`])。
pub fn raw_shortcut_codes() -> BTreeSet<KeySequence> {
    FIXED_FIRST_SHORTCUTS_TSV
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let mut fields = line.split('\t');
            fields.next();
            fields.next();
            fields
                .next()
                .expect("FF 数据行应有 shortcut 字段")
                .parse()
                .expect("FF shortcut 码应可解析")
        })
        .collect()
}

/// 初始化安全的 merged-ranking 原始视图:`(词, shortcut, 绝对 rank=1)`。
/// 不调用 canonical 单字/词层,避免最终化过程中的 OnceLock 初始化环。
pub(crate) fn raw_ranking_entries() -> Vec<(String, KeySequence, usize)> {
    FIXED_FIRST_SHORTCUTS_TSV
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            let word = fields.first().expect("FIXED_FIRST 应有词字段").to_string();
            let code = fields
                .get(2)
                .expect("FIXED_FIRST 应有 shortcut 字段")
                .parse()
                .expect("FIXED_FIRST shortcut 应合法");
            (word, code, 1)
        })
        .collect()
}

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

    /// Rime 静态同码全组的唯一整数权重(本层绝对 rank 恒为 1)。
    pub fn rime_weight(&self) -> u32 {
        crate::merged_ranking::merged_weight(&self.shortcut_code, &self.word)
            .expect("FIXED_FIRST 条目必须参与 merged ranking")
    }
}

/// 全部 canonical FIXED_FIRST 词语简码关系(进程内共享,解析一次;
/// canonical 序列化顺序)。
pub fn canonical_fixed_first_shortcut_entries() -> &'static [CanonicalFixedFirstShortcutEntry] {
    static ENTRIES: OnceLock<Vec<CanonicalFixedFirstShortcutEntry>> = OnceLock::new();
    ENTRIES
        .get_or_init(|| {
            parse_tsv(
                FIXED_FIRST_SHORTCUTS_TSV,
                "word_fixed_first.tsv",
                DatasetKind::CurrentV2,
            )
        })
        .as_slice()
}

/// v1 selector 的冻结 FIXED_FIRST 关系。
///
/// 仅供 analyzer 的 legacy/research-only 重放;production generator 绝不读取。
pub fn legacy_v1_fixed_first_shortcut_entries() -> &'static [CanonicalFixedFirstShortcutEntry] {
    static ENTRIES: OnceLock<Vec<CanonicalFixedFirstShortcutEntry>> = OnceLock::new();
    ENTRIES
        .get_or_init(|| {
            parse_tsv(
                LEGACY_V1_FIXED_FIRST_SHORTCUTS_TSV,
                "legacy/word_fixed_first_v1.tsv",
                DatasetKind::LegacyV1,
            )
        })
        .as_slice()
}

#[derive(Clone, Copy)]
enum DatasetKind {
    CurrentV2,
    LegacyV1,
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
fn parse_tsv(
    text: &'static str,
    name: &str,
    dataset_kind: DatasetKind,
) -> Vec<CanonicalFixedFirstShortcutEntry> {
    // 固定词层的 (词, 完整码) 成员资格与 baseline 组。
    let word_codes: BTreeSet<(String, String)> = crate::canonical_word_code_entries()
        .iter()
        .map(|entry| (entry.word().to_string(), entry.code().to_string()))
        .collect();
    let baseline_groups = baseline_fixed_groups();
    // 跨层集合只做原始文本扫描,避免兄弟 OnceLock 循环初始化。
    let (forbidden_words, forbidden_codes, forbidden_layer): (
        BTreeSet<&str>,
        BTreeSet<KeySequence>,
        &str,
    ) = match dataset_kind {
        DatasetKind::CurrentV2 => (
            crate::primary_shortcuts::raw_words(),
            BTreeSet::new(),
            "PRIMARY",
        ),
        DatasetKind::LegacyV1 => {
            let mut words = crate::word_shortcuts::raw_words();
            words.extend(crate::two_key_shortcuts::raw_words());
            (
                words,
                crate::word_shortcuts::raw_shortcut_codes(),
                "legacy ZERO_REGRESSION/二码",
            )
        }
    };

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

        // 词:2~4 个规范汉字;不得同时持有 PRIMARY 简码。
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
            !forbidden_words.contains(word),
            "{name} 第 {row_number} 行词已持有 {forbidden_layer} 简码: {word:?}"
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

        // shortcut:纯小写 a-z,长度 ≥ 3 且小于完整码。
        let shortcut_code: KeySequence = shortcut_field
            .parse()
            .unwrap_or_else(|_| panic!("{name} 第 {row_number} 行 shortcut 码非法: {line:?}"));
        assert!(
            shortcut_code.len() >= 3 && shortcut_code.len() < full_code.len(),
            "{name} 第 {row_number} 行 shortcut 长度应在 [3, 完整码) 内: {line:?}"
        );
        assert!(
            !forbidden_codes.contains(&shortcut_code),
            "{name} 第 {row_number} 行 shortcut 与 {forbidden_layer} 码冲突: {line:?}"
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
            entries.len() < crate::canonical_primary_shortcut_entries().len(),
            "FIXED_FIRST 层应显著小于 PRIMARY 层"
        );
    }
}
