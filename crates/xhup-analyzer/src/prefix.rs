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
use xhup_generator::{
    canonical_fixed_first_shortcut_entries, canonical_word_shortcut_entries,
    word_code_analysis_entries,
};

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

/// 一个 FIXED_FIRST runtime continuation 哨兵:FF shortcut 是某更长合法码
/// 的 strict prefix,继续输入剩余键必须能抵达更长码目标。
pub struct FixedFirstPrefixSentinel {
    /// 词语。
    pub word: String,
    /// 完整码(保留可用)。
    pub full_code: KeySequence,
    /// FF shortcut 码。
    pub shortcut_code: KeySequence,
    /// F/I 投影模式。
    pub mode: String,
    /// 要继续输入到的更长合法码(字典序最小者)。
    pub longer_code: KeySequence,
    /// 更长码的期望目标:若更长码恰为自身完整码则是词本身,否则为
    /// current production 组内首候选。
    pub longer_target: String,
}

/// 单个 FIXED_FIRST 码长层(3~7 键)的 continuation 哨兵选择。
pub struct FixedFirstLengthSentinel {
    /// 码长。
    pub length: usize,
    /// 该层条数(0 时 continuation 为 None)。
    pub rows: usize,
    /// 字典序首条「FF shortcut 是某更长合法码 strict prefix」(runtime
    /// prefix continuation 哨兵)。
    pub continuation: Option<FixedFirstPrefixSentinel>,
}

/// FIXED_FIRST 简码层的 prefix 拓扑全量静态审计结果。
pub struct FixedFirstPrefixAudit {
    /// production FIXED_FIRST 简码总数。
    pub shortcut_count: usize,
    /// A:FF shortcut 是某更长合法码(baseline / ZR / FF)strict prefix 的
    /// (shortcut, 更长码) 对数。
    pub shortcut_prefix_of_longer_pairs: usize,
    /// A 中涉及的 distinct FF shortcut 数。
    pub shortcuts_prefixing_longer: usize,
    /// B:更短合法码(baseline / ZR / FF)是某 FF shortcut strict prefix 的对数。
    pub shorter_prefix_of_shortcut_pairs: usize,
    /// C/D:FF shortcut 互为 strict prefix 的对数(方向唯一:短者 → 长者)。
    pub shortcut_to_shortcut_pairs: usize,
    /// reverse prefix runtime 代表案例:(更短合法码, FF shortcut, 词)。
    pub reverse_example: Option<(KeySequence, KeySequence, String)>,
    /// 3~7 键各层 continuation 哨兵。
    pub lengths: Vec<FixedFirstLengthSentinel>,
}

/// 对全部 production FIXED_FIRST 简码做 prefix 拓扑审计,并按层导出
/// deterministic runtime continuation 哨兵。
///
/// `current` 必须是 current production occupancy(baseline + ZR + FF,
/// 即「更长/更短合法码」的全集)。
pub fn audit_fixed_first_prefix_topology(current: &CodeOccupancy) -> FixedFirstPrefixAudit {
    let entries = canonical_fixed_first_shortcut_entries();
    let ff_set: std::collections::BTreeSet<&KeySequence> =
        entries.iter().map(|e| e.shortcut_code()).collect();
    let current_set: std::collections::BTreeSet<&KeySequence> = current.occupied_codes().collect();

    // A:枚举每个更长合法码的 strict prefix(≥3 键才可能命中 FF 层),
    // 查 FF 集合;同时记录每个 FF 码的更长扩展(字典序由集合序保证)。
    let mut shortcut_prefix_of_longer_pairs = 0usize;
    let mut extensions: BTreeMap<&KeySequence, Vec<&KeySequence>> = BTreeMap::new();
    for code in &current_set {
        let keys = code.as_slice();
        for k in 3..keys.len() {
            let prefix = KeySequence::from_keys(&keys[..k]).expect("prefix 非空");
            if let Some(&shortcut) = ff_set.get(&prefix) {
                shortcut_prefix_of_longer_pairs += 1;
                extensions.entry(shortcut).or_default().push(code);
            }
        }
    }

    // B 与 C/D:枚举每个 FF shortcut 的全部 strict prefix。
    let mut shorter_prefix_of_shortcut_pairs = 0usize;
    let mut shortcut_to_shortcut_pairs = 0usize;
    let mut reverse_example: Option<(KeySequence, KeySequence, String)> = None;
    for entry in entries {
        let keys = entry.shortcut_code().as_slice();
        for k in 1..keys.len() {
            let prefix = KeySequence::from_keys(&keys[..k]).expect("prefix 非空");
            if current_set.contains(&prefix) {
                shorter_prefix_of_shortcut_pairs += 1;
                if reverse_example.is_none() {
                    reverse_example = Some((
                        prefix.clone(),
                        entry.shortcut_code().clone(),
                        entry.word().to_string(),
                    ));
                }
            }
            if ff_set.contains(&prefix) {
                shortcut_to_shortcut_pairs += 1;
            }
        }
    }

    // 每层 continuation 哨兵:canonical 顺序遍历,首个有更长扩展的条目,
    // 更长码取字典序最小者。
    let mut lengths: Vec<FixedFirstLengthSentinel> = (3..=7)
        .map(|length| FixedFirstLengthSentinel {
            length,
            rows: 0,
            continuation: None,
        })
        .collect();
    for entry in entries {
        let length = entry.shortcut_code().len();
        let slot = &mut lengths[length - 3];
        slot.rows += 1;
        if slot.continuation.is_some() {
            continue;
        }
        let Some(longer_codes) = extensions.get(entry.shortcut_code()) else {
            continue;
        };
        let longer_code = (*longer_codes.first().expect("扩展非空")).clone();
        let longer_target = if longer_code == *entry.full_code() {
            entry.word().to_string()
        } else {
            current
                .group(&longer_code)
                .and_then(|group| group.first())
                .expect("更长码组必然存在")
                .text()
                .to_string()
        };
        slot.continuation = Some(FixedFirstPrefixSentinel {
            word: entry.word().to_string(),
            full_code: entry.full_code().clone(),
            shortcut_code: entry.shortcut_code().clone(),
            mode: entry.mode().to_string(),
            longer_code,
            longer_target,
        });
    }

    FixedFirstPrefixAudit {
        shortcut_count: entries.len(),
        shortcut_prefix_of_longer_pairs,
        shortcuts_prefixing_longer: extensions.len(),
        shorter_prefix_of_shortcut_pairs,
        shortcut_to_shortcut_pairs,
        reverse_example,
        lengths,
    }
}
