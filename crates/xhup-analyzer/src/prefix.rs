//! 词语简码层的 prefix 拓扑全量静态审计。
//!
//! 已入库的词语简码占据原本空闲的 3~7 键 exact-code 空间,其中部分码会是
//! 更长合法码的 strict prefix。这些关系本身不是错误(table_translator
//! 天然支持 exact 候选与更长输入共存),但 runtime 冒烟测试不可能覆盖
//! 数万条简码,因此这里做全量静态审计,并按确定性规则导出每层的
//! runtime 哨兵(prefix continuation / 非 prefix alias / 高频代表)。
//!
//! 本模块只做静态分析,不读写文件。

use std::collections::BTreeMap;

use xhup_core::KeySequence;
use xhup_generator::{canonical_word_shortcut_entries, word_code_analysis_entries};

use crate::occupancy::CodeOccupancy;

/// 一个 runtime 哨兵:canonical 集中按确定规则选出的代表条目。
pub struct PrefixSentinel {
    /// 词语。
    pub word: String,
    /// 完整码(保留可用;prefix continuation 的继续输入目标)。
    pub full_code: KeySequence,
    /// shortcut 码。
    pub shortcut_code: KeySequence,
    /// F/I 投影模式。
    pub mode: String,
    /// 万象聚合频率分数。
    pub frequency_score: u64,
}

/// 单个简码码长层(3~7 键)的哨兵选择。
pub struct LengthSentinels {
    /// 码长。
    pub length: usize,
    /// 该层条数(0 时其余字段均为 None)。
    pub rows: usize,
    /// 字典序首条(exact 输入哨兵)。
    pub lex_first: Option<PrefixSentinel>,
    /// 频率最高条(exact 输入哨兵)。
    pub top_frequency: Option<PrefixSentinel>,
    /// 字典序首条「shortcut 是自身完整码 strict prefix」(prefix continuation 哨兵)。
    pub prefix_lex_first: Option<PrefixSentinel>,
    /// 字典序首条「shortcut 不是自身完整码 prefix」(非 prefix alias 哨兵)。
    pub non_prefix_lex_first: Option<PrefixSentinel>,
}

/// prefix 拓扑全量审计结果。
pub struct PrefixAudit {
    /// production 简码总数。
    pub shortcut_count: usize,
    /// A:shortcut 是某 baseline 固定码 strict prefix 的 (shortcut, baseline) 对数。
    pub shortcut_prefix_of_baseline_pairs: usize,
    /// A 中涉及的 distinct shortcut 数。
    pub shortcuts_prefixing_baseline: usize,
    /// B:baseline 固定码是某 shortcut strict prefix 的 (baseline, shortcut) 对数。
    pub baseline_prefix_of_shortcut_pairs: usize,
    /// C/D:shortcut 互为 strict prefix 的对数(方向唯一:短者 → 长者)。
    pub shortcut_to_shortcut_pairs: usize,
    /// 3~7 键各层哨兵(无该层的码长 rows 为 0)。
    pub lengths: Vec<LengthSentinels>,
}

/// `prefix` 是否为 `code` 的 strict prefix。
fn is_strict_prefix(prefix: &KeySequence, code: &KeySequence) -> bool {
    prefix.len() < code.len() && code.as_slice().starts_with(prefix.as_slice())
}

/// 对全部 production 简码做 prefix 拓扑审计,并按层导出 deterministic 哨兵。
///
/// `baseline` 必须是 baseline fixed occupancy(不含简码层本身)。
pub fn audit_prefix_topology(baseline: &CodeOccupancy) -> PrefixAudit {
    let entries = canonical_word_shortcut_entries();
    let shortcut_set: std::collections::BTreeSet<&KeySequence> =
        entries.iter().map(|e| e.shortcut_code()).collect();
    let baseline_set: std::collections::BTreeSet<&KeySequence> =
        baseline.occupied_codes().collect();

    // 频率证据 join:(词, 完整码) → 频率分数。
    let word_entries = word_code_analysis_entries();
    let word_scores: BTreeMap<(&str, &KeySequence), u64> = word_entries
        .iter()
        .map(|entry| ((entry.word(), entry.code()), entry.frequency_score()))
        .collect();

    // A:枚举每个 baseline 码的全部 strict prefix,查 shortcut 集合。
    let mut shortcut_prefix_of_baseline_pairs = 0usize;
    let mut prefixing_shortcuts: std::collections::BTreeSet<&KeySequence> =
        std::collections::BTreeSet::new();
    for code in &baseline_set {
        let keys = code.as_slice();
        for k in 1..keys.len() {
            let prefix = KeySequence::from_keys(&keys[..k]).expect("prefix 非空");
            if let Some(&shortcut) = shortcut_set.get(&prefix) {
                shortcut_prefix_of_baseline_pairs += 1;
                prefixing_shortcuts.insert(shortcut);
            }
        }
    }

    // B 与 C/D:枚举每个 shortcut 的全部 strict prefix。
    let mut baseline_prefix_of_shortcut_pairs = 0usize;
    let mut shortcut_to_shortcut_pairs = 0usize;
    for entry in entries {
        let keys = entry.shortcut_code().as_slice();
        for k in 1..keys.len() {
            let prefix = KeySequence::from_keys(&keys[..k]).expect("prefix 非空");
            if baseline_set.contains(&prefix) {
                baseline_prefix_of_shortcut_pairs += 1;
            }
            if shortcut_set.contains(&prefix) {
                shortcut_to_shortcut_pairs += 1;
            }
        }
    }

    // 每层哨兵:canonical 顺序即 (码长, 码字典序, …),遍历一次取首条;
    // 高频哨兵按频率分数降序(同分取字典序靠前者,确定)。
    let mut lengths: Vec<LengthSentinels> = (3..=7)
        .map(|length| LengthSentinels {
            length,
            rows: 0,
            lex_first: None,
            top_frequency: None,
            prefix_lex_first: None,
            non_prefix_lex_first: None,
        })
        .collect();
    for entry in entries {
        let length = entry.shortcut_code().len();
        let slot = &mut lengths[length - 3];
        slot.rows += 1;
        let sentinel = |frequency_score: u64| PrefixSentinel {
            word: entry.word().to_string(),
            full_code: entry.full_code().clone(),
            shortcut_code: entry.shortcut_code().clone(),
            mode: entry.mode().to_string(),
            frequency_score,
        };
        let frequency_score = *word_scores
            .get(&(entry.word(), entry.full_code()))
            .expect("词语简码的 (词, 完整码) 必须存在于固定词层");
        if slot.lex_first.is_none() {
            slot.lex_first = Some(sentinel(frequency_score));
        }
        let is_own_prefix = is_strict_prefix(entry.shortcut_code(), entry.full_code());
        if is_own_prefix && slot.prefix_lex_first.is_none() {
            slot.prefix_lex_first = Some(sentinel(frequency_score));
        }
        if !is_own_prefix && slot.non_prefix_lex_first.is_none() {
            slot.non_prefix_lex_first = Some(sentinel(frequency_score));
        }
        let replace = slot
            .top_frequency
            .as_ref()
            .is_none_or(|current| frequency_score > current.frequency_score);
        if replace {
            slot.top_frequency = Some(sentinel(frequency_score));
        }
    }

    PrefixAudit {
        shortcut_count: entries.len(),
        shortcut_prefix_of_baseline_pairs,
        shortcuts_prefixing_baseline: prefixing_shortcuts.len(),
        baseline_prefix_of_shortcut_pairs,
        shortcut_to_shortcut_pairs,
        lengths,
    }
}
