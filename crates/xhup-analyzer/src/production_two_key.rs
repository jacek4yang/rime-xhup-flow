//! Production 二码零冲突词语简码:仅对 2 键 exact-code 空间**完全空闲**
//! 的 `II` 理论候选建立生产层(`two-key-zero-regression-v1`)。
//!
//! policy `two-key-zero-regression-v1`:
//!
//! ```text
//! candidate grammar  = MonotoneSuffixInitialsV2(2 字词 × II × 2 键)
//! target universe    = 无既有 ZR/FF 简码的 2 字词(Universe A)
//! candidate universe = 2 键 exact-code 空间空闲(char fanout == 0,
//!                      且无任何既有 production exact 关系)
//! reference run      = OperatingPointId::Balanced × normalized(50:50,Conservative)
//! robustness         = 30 次研究网格中同词胜出票数,
//!                      整数交叉乘法 votes × 5 >= total_runs × 4
//! net gain           = reference 运行下有效收益 > 0
//! 竞争               = 每 2 键码恰一词,确定性排序:
//!                      净收益 DESC → 频率 DESC → 词 ASC → 完整码 ASC
//! 兼容               = 与既有全部 production 关系全量不相交
//! ```
//!
//! 与 ZR/FF 层的本质差别:本层候选的码在当前 exact-code 空间完全空闲
//! (连 2 键单字都没有),所以新词将是该码唯一的 exact 候选(rank 1),
//! 不与任何既有候选共享码位 —— 严格 zero-regression。占用码
//! (char fanout > 0)的 SAFE_APPEND / OPTIMAL_INSERT 只存在于研究
//! 报告(`two_key_study`),永不由本 policy 生产化。

use std::collections::{BTreeMap, BTreeSet};

use xhup_core::KeySequence;

use crate::candidates::CandidateGrammar;
use crate::frequency::{CharCodeUsage, FrequencyScale};
use crate::production::{ROBUSTNESS_DENOMINATOR, ROBUSTNESS_NUMERATOR};
use crate::sweep::OperatingPointId;
use crate::two_key_study::{TwoKeyStudyRun, TwoKeyUniverse, reference_run_index};

/// Production 二码零冲突 selection policy 的稳定版本标识(写入 canonical TSV 头)。
pub const TWO_KEY_PRODUCTION_POLICY_VERSION: &str = "two-key-zero-regression-v1";

/// 一条 production 二码零冲突简码关系(canonical TSV 行 + audit 字段)。
#[derive(Clone, Debug)]
pub struct TwoKeySelection {
    /// 词语(2 字)。
    pub word: String,
    /// 完整码(4 键,保留别名)。
    pub full_code: KeySequence,
    /// 2 键 II 简码(= reference 运行的获胜码)。
    pub shortcut_code: KeySequence,
    /// 模式(恒 `II`)。
    pub mode: &'static str,
    /// 万象聚合频率分数(仅 audit)。
    pub frequency_score: u64,
    /// 当前最优静态路径的有效成本(仅 audit)。
    pub current_best_cost: f64,
    /// 2 键路径有效成本(rank 1, fanout 1;仅 audit)。
    pub two_key_cost: f64,
    /// reference 频率加权净收益(仅 audit)。
    pub weighted_net_gain: f64,
    /// 30 次网格中同码同词胜出的票数。
    pub top_word_votes: usize,
    /// 网格总运行数(恒 30)。
    pub total_runs: usize,
}

/// 二码零冲突层的排除原因(选择 audit)。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum TwoKeyExclusionReason {
    /// 码已被既有 production 关系占用(char fanout > 0 或其它 exact 关系)。
    CodeOccupied,
    /// 词已有 ZR/FF 简码(Universe A 之外)。
    WordHasExistingShortcut,
    /// reference 运行净收益非正。
    NonPositiveReferenceUtility,
    /// 同码竞争中未获胜(每码恰一词)。
    LostPerCodeCompetition,
    /// 同码胜出词在网格中不稳定(票数 < 4/5)。
    BelowRobustnessThreshold,
    /// 稳定胜出词与 reference 胜出词不一致。
    WinnerMismatch,
}

/// 选择 audit 计数。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TwoKeySelectionAudit {
    /// Universe A(无既有简码)中的空码候选数。
    pub empty_code_candidates: usize,
    /// 进入 production 的条数。
    pub selected: usize,
    /// 各排除原因的条数(候选级)。
    pub excluded: BTreeMap<TwoKeyExclusionReason, usize>,
    /// 涉及竞争的空码数(≥ 2 个候选)。
    pub contested_codes: usize,
}

/// 同码竞争的确定性偏序:「A 优于 B」的判定(供 `max_by` 使用,
/// 返回 `Ordering::Greater` 表示 A 更优)。
///
/// 文档顺序:净收益 DESC → 频率 DESC → 词 ASC → 完整码 ASC。
/// 逐级展开为显式比较,避免 `.reverse()` 链,便于 review:
///
/// - 净收益:A > B → A 优(gain 主序,任何平局落到下一级);
/// - 频率:A > B → A 优(高频优先);
/// - 词:A < B → A 优(字典序小者优先);
/// - 完整码:A < B → A 优(字典序小者优先)。
fn compare_candidates(
    gain_a: f64,
    candidate_a: &crate::two_key_study::TwoKeyCandidate,
    gain_b: f64,
    candidate_b: &crate::two_key_study::TwoKeyCandidate,
) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    // 净收益 DESC:A 更大 → A 更优(Greater)。
    let gain = gain_a.partial_cmp(&gain_b).expect("净收益非 NaN");
    if gain != Ordering::Equal {
        return gain;
    }
    // 频率 DESC:A 更大 → A 更优(Greater)。
    let frequency = candidate_a
        .frequency_score
        .cmp(&candidate_b.frequency_score);
    if frequency != Ordering::Equal {
        return frequency;
    }
    // 词 ASC:A 更小 → A 更优(Greater)。
    let word = candidate_a.word.cmp(&candidate_b.word);
    if word != Ordering::Equal {
        return word.reverse();
    }
    // 完整码 ASC:A 更小 → A 更优(Greater)。
    candidate_a.full_code.cmp(&candidate_b.full_code).reverse()
}

/// 从研究证据中选择 production 二码零冲突集。
///
/// 硬不变量(违反即 STOP):
/// - 每个入选码在当前 production occupancy 中 fanout == 0(独立重验);
/// - 每词/每码/每 (词, 完整码) 唯一;
/// - 机械 II 投影一致;
/// - 全部入选词属于 Universe A。
pub fn select_two_key_production(
    universe: &TwoKeyUniverse,
    runs: &[TwoKeyStudyRun],
) -> (Vec<TwoKeySelection>, TwoKeySelectionAudit) {
    let reference = reference_run_index(runs);
    let total_runs = runs.len();
    let mut audit = TwoKeySelectionAudit::default();

    // Universe A × 空码 候选,按码分组。
    let mut by_code: BTreeMap<&KeySequence, Vec<usize>> = BTreeMap::new();
    for (index, candidate) in universe.candidates.iter().enumerate() {
        if !candidate.existing_shortcut.is_none() {
            continue;
        }
        if candidate.char_fanout > 0 {
            continue;
        }
        // 独立重验:当前 production occupancy 中该码必须完全空闲
        //(不仅无 2 键单字,且无任何既有 exact 关系)。
        assert_eq!(
            universe.occupancy.fanout(&candidate.two_key_code),
            0,
            "空码候选在 production occupancy 中必须 fanout == 0: {}",
            candidate.two_key_code
        );
        audit.empty_code_candidates += 1;
        by_code
            .entry(&candidate.two_key_code)
            .or_default()
            .push(index);
    }
    audit.contested_codes = by_code.values().filter(|v| v.len() > 1).count();

    // 每码竞争:网格中统计同码获胜词(净收益最大;确定性平局序),
    // 要求整数 4/5 稳定票。
    let mut selected = Vec::new();
    for indices in by_code.values() {
        // 每次网格运行:该码的获胜词(`compare_candidates` 偏序的最大者;
        // 文档顺序:净收益 DESC → 频率 DESC → 词 ASC → 完整码 ASC)。
        let winner_of_run = |run: &TwoKeyStudyRun| -> usize {
            *indices
                .iter()
                .max_by(|&&a, &&b| {
                    compare_candidates(
                        run.safe[a].weighted_net_gain,
                        &universe.candidates[a],
                        run.safe[b].weighted_net_gain,
                        &universe.candidates[b],
                    )
                })
                .expect("码组非空")
        };
        let reference_winner = winner_of_run(&runs[reference]);
        let reference_evaluation = &runs[reference].safe[reference_winner];
        let candidate = &universe.candidates[reference_winner];

        // reference 净收益必须为正。
        if reference_evaluation.weighted_net_gain <= 0.0 {
            *audit
                .excluded
                .entry(TwoKeyExclusionReason::NonPositiveReferenceUtility)
                .or_default() += indices.len();
            continue;
        }
        // 网格稳定票:同码同词胜出次数 >= 4/5。
        let votes = runs
            .iter()
            .filter(|run| winner_of_run(run) == reference_winner)
            .count();
        if votes * ROBUSTNESS_DENOMINATOR < total_runs * ROBUSTNESS_NUMERATOR {
            *audit
                .excluded
                .entry(TwoKeyExclusionReason::BelowRobustnessThreshold)
                .or_default() += 1;
            continue;
        }
        // 每码恰一词:落败候选计数(LostPerCodeCompetition)。
        if indices.len() > 1 {
            *audit
                .excluded
                .entry(TwoKeyExclusionReason::LostPerCodeCompetition)
                .or_default() += indices.len() - 1;
        }
        selected.push(TwoKeySelection {
            word: candidate.word.clone(),
            full_code: candidate.full_code.clone(),
            shortcut_code: candidate.two_key_code.clone(),
            mode: "II",
            frequency_score: candidate.frequency_score,
            current_best_cost: candidate.current_best.cost.total(),
            two_key_cost: reference_evaluation.cost.total(),
            weighted_net_gain: reference_evaluation.weighted_net_gain,
            top_word_votes: votes,
            total_runs,
        });
    }

    // 唯一不变量(词/码/(词,完整码))。
    let mut words = BTreeSet::new();
    let mut codes = BTreeSet::new();
    let mut word_full_codes = BTreeSet::new();
    for entry in &selected {
        assert!(words.insert(entry.word.as_str()), "production 词重复");
        assert!(
            codes.insert(entry.shortcut_code.clone()),
            "production 码重复"
        );
        assert!(
            word_full_codes.insert((entry.word.as_str(), entry.full_code.clone())),
            "production (词, 完整码) 重复"
        );
        // 机械 II 投影重验。
        let full = entry.full_code.as_slice();
        assert_eq!(
            entry.shortcut_code.as_slice(),
            &[full[0], full[2]],
            "II 投影不一致: {}",
            entry.word
        );
    }
    // canonical 序列化顺序:码 → 词 → 完整码(全部 2 键,长度无差异)。
    selected.sort_by(|a, b| {
        a.shortcut_code
            .cmp(&b.shortcut_code)
            .then(a.word.cmp(&b.word))
            .then(a.full_code.cmp(&b.full_code))
    });
    audit.selected = selected.len();
    (selected, audit)
}

/// 序列化为 canonical TSV:`词<TAB>完整码<TAB>shortcut 码<TAB>模式`。
///
/// UTF-8、LF、无 BOM、恰好一个末尾换行;`#` 头记录 policy / 语法 /
/// 网格 / 门槛 provenance,不含时间戳、主机、路径。utility、票数等
/// analysis evidence 不属于 production semantic identity,不写入。
pub fn serialize_two_key_tsv(selected: &[TwoKeySelection]) -> String {
    let mut out = String::new();
    out.push_str("# XHUP Flow two-key zero-regression word shortcuts.\n");
    out.push_str("# Source universe: data/words/wanxiang_base_words.tsv\n");
    out.push_str(
        "# Existing production words excluded: word_zero_regression.tsv + word_fixed_first.tsv\n",
    );
    out.push_str(
        "# Selection: EMPTY 2-key exact codes only (char fanout == 0) / balanced / normalized 50:50 conservative\n",
    );
    out.push_str("# candidate grammar: ");
    out.push_str(CandidateGrammar::MonotoneSuffixInitialsV2.label());
    out.push('\n');
    out.push_str(
        "# Per-code winner stability >= 4/5 over 30 sensitivity runs (integer cross-multiply)\n",
    );
    out.push_str("# policy: ");
    out.push_str(TWO_KEY_PRODUCTION_POLICY_VERSION);
    out.push('\n');
    for entry in selected {
        out.push_str(&entry.word);
        out.push('\t');
        out.push_str(&entry.full_code.to_string());
        out.push('\t');
        out.push_str(&entry.shortcut_code.to_string());
        out.push('\t');
        out.push_str(entry.mode);
        out.push('\n');
    }
    out
}

/// 参考尺度(与 ZR/FF 层相同:balanced × 50:50 × conservative)。
pub fn reference_scale() -> FrequencyScale {
    FrequencyScale::Normalized {
        char_share: 0.5,
        usage: CharCodeUsage::Conservative,
    }
}

/// 参考 operating point typed identity。
pub fn reference_point() -> OperatingPointId {
    OperatingPointId::Balanced
}

/// 二码零冲突层的频率加权收益审计(reference 尺度)。
#[derive(Clone, Copy, Debug, Default)]
pub struct TwoKeyBenefitAudit {
    /// production 集的 raw 加权按键节省(纯键数差)。
    pub raw_keys_saved: f64,
    /// production 集的有效模型收益(净收益合计)。
    pub effective_benefit: f64,
}

/// 计算收益审计(权重 = reference 尺度 word 域)。
pub fn two_key_benefit_audit(
    universe: &TwoKeyUniverse,
    selected: &[TwoKeySelection],
) -> TwoKeyBenefitAudit {
    let scale = reference_scale();
    let raw: f64 = selected
        .iter()
        .map(|e| {
            universe.frequency.target_weight(&scale, e.frequency_score)
                * (e.full_code.len() - e.shortcut_code.len()) as f64
        })
        .sum();
    let effective: f64 = selected.iter().map(|e| e.weighted_net_gain).sum();
    TwoKeyBenefitAudit {
        raw_keys_saved: raw,
        effective_benefit: effective,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::two_key_study::{ExistingShortcutStatus, StaticRoute, TwoKeyCodeClass};
    use xhup_core::KeySequence;

    /// 合成候选(仅排序所需字段;winner 竞争只读 word/full_code/frequency)。
    fn synthetic(
        word: &str,
        full_code: &str,
        frequency_score: u64,
    ) -> crate::two_key_study::TwoKeyCandidate {
        let full_code: KeySequence = full_code.parse().expect("合成完整码合法");
        let keys = full_code.as_slice();
        let two_key_code = KeySequence::from_keys(&[keys[0], keys[2]]).expect("II 码合法");
        crate::two_key_study::TwoKeyCandidate {
            word: word.to_string(),
            full_code: full_code.clone(),
            two_key_code,
            frequency_score,
            existing_shortcut: ExistingShortcutStatus::None,
            current_best: StaticRoute {
                kind: crate::two_key_study::RouteKind::FullCode,
                code: full_code,
                rank: 1,
                fanout: 1,
                cost: crate::cost::CostBreakdown::default(),
            },
            code_class: TwoKeyCodeClass::Empty,
            char_fanout: 0,
        }
    }

    /// `compare_candidates` 偏序:P0 回归锁(review 修正前的实现曾把
    /// 频率/词/完整码三个平局级方向全部接反)。
    #[test]
    fn comparator_implements_documented_ordering() {
        use std::cmp::Ordering;

        // Case 1 — 频率:同码同净收益,高频者胜。
        let a = synthetic("甲词", "abcd", 100);
        let b = synthetic("乙词", "abce", 50);
        assert_eq!(
            compare_candidates(1.0, &a, 1.0, &b),
            Ordering::Greater,
            "同净收益:频率 100 应胜 50"
        );
        assert_eq!(
            compare_candidates(1.0, &b, 1.0, &a),
            Ordering::Less,
            "反方向:频率 50 应输 100"
        );

        // Case 2 — 词字典序:同码同净收益同频率,字典序小者胜。
        // (乙 U+4E59 < 甲 U+7532,真实 Unicode 标量序。)
        let a = synthetic("乙词", "abcd", 100);
        let b = synthetic("甲词", "abce", 100);
        assert_eq!(
            compare_candidates(1.0, &a, 1.0, &b),
            Ordering::Greater,
            "同净收益同频率:词「乙词」应胜「甲词」(字典序小者优先)"
        );
        assert_eq!(
            compare_candidates(1.0, &b, 1.0, &a),
            Ordering::Less,
            "反方向:「甲词」应输「乙词」"
        );

        // Case 3 — 完整码字典序:同净收益同频率同词(多音词语境;
        // 当前 production universe 中一词一候选,故直接测比较器),
        // 完整码字典序小者胜。
        let a = synthetic("同词", "abcd", 100);
        let b = synthetic("同词", "abce", 100);
        assert_eq!(
            compare_candidates(1.0, &a, 1.0, &b),
            Ordering::Greater,
            "同词同净收益同频率:完整码 abcd 应胜 abce"
        );
        assert_eq!(
            compare_candidates(1.0, &b, 1.0, &a),
            Ordering::Less,
            "反方向:完整码 abce 应输 abcd"
        );

        // Case 4 — 净收益主序:低频候选只要净收益严格更大仍然胜出,
        // 证明频率不会覆盖净收益。
        let low_freq_high_gain = synthetic("低频", "abcd", 50);
        let high_freq_low_gain = synthetic("高频", "abce", 100);
        assert_eq!(
            compare_candidates(2.0, &low_freq_high_gain, 1.0, &high_freq_low_gain),
            Ordering::Greater,
            "净收益 2.0(频率 50)必须胜 净收益 1.0(频率 100)"
        );
        assert_eq!(
            compare_candidates(1.0, &high_freq_low_gain, 2.0, &low_freq_high_gain),
            Ordering::Less,
            "反方向:净收益 1.0 必须输 净收益 2.0"
        );

        // 全平:等价(不偏向任一方;max_by 语义下取先出现者,确定性)。
        let a = synthetic("同词", "abcd", 100);
        let b = synthetic("同词", "abcd", 100);
        assert_eq!(compare_candidates(1.0, &a, 1.0, &b), Ordering::Equal);
    }

    /// `max_by` 与比较器的组合语义:同码组中按文档顺序取出唯一胜者。
    /// 用 3 个合成候选验证「频率 DESC」在 max_by 下方向正确
    ///(review 前的实现在此方向上是反的,可能选中低频词)。
    #[test]
    fn max_by_picks_higher_frequency_on_equal_gain() {
        let low_freq = synthetic("高频词占位", "abcd", 50);
        let high_freq = synthetic("另一词", "abce", 100);
        let group = [low_freq, high_freq];
        let winner = group
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| compare_candidates(1.0, a, 1.0, b))
            .map(|(i, _)| i)
            .expect("组非空");
        assert_eq!(
            group[winner].frequency_score, 100,
            "max_by 必须选中频率 100 的候选(频率 DESC)"
        );
    }
}
