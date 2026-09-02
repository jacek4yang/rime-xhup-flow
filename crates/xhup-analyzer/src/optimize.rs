//! 确定性贪心优化器:评分 → 丢弃非正收益 → 全序排序 → word/code 双唯一分配。
//!
//! 这是 deterministic heuristic,不声称数学全局最优。四个 profile:
//!
//! ```text
//! EMPTY_LENGTH_ONLY  只允许 5/7 键 shortcut(当前整体空闲码长层),exact code 必须空闲
//! ZERO_REGRESSION    允许 len 3..7,exact code 必须空闲(zero exact-code regression:
//!                    不重排任何已有 exact 候选组;不证明 Rime runtime 绝对零行为变化)
//! FIXED_FIRST        shortcut 追加到所有现有固定候选之后,既有排名扰动恒为 0
//! OPTIMIZED          纯分析模拟:shortcut 按混合频率参与排名;硬保护 1 键 immutable、
//!                    2 键保留(shortcut 长度 ≥3 不可达)、4 键规范全码 top 不被词语挤掉
//! ```

use std::collections::{BTreeMap, BTreeSet};

use xhup_core::KeySequence;

use crate::candidates::{ShortcutCandidate, WordTarget};
use crate::cost::{CostBreakdown, CostModel};
use crate::frequency::{FrequencyModel, FrequencyScale};
use crate::occupancy::{CandidateSource, CodeOccupancy, CollisionClass};

/// 优化 profile。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum OptimizationProfile {
    /// 仅利用整体空闲的 5/7 键码长层(audit profile)。
    EmptyLengthOnly,
    /// 仅允许 exact code 当前空闲的 shortcut。
    ZeroRegression,
    /// shortcut 追加到现有固定候选之后。
    FixedFirst,
    /// 分析性重排模拟(带硬保护)。
    Optimized,
}

impl OptimizationProfile {
    /// 报告用稳定标签。
    pub fn label(self) -> &'static str {
        match self {
            OptimizationProfile::EmptyLengthOnly => "EMPTY_LENGTH_ONLY",
            OptimizationProfile::ZeroRegression => "ZERO_REGRESSION",
            OptimizationProfile::FixedFirst => "FIXED_FIRST",
            OptimizationProfile::Optimized => "OPTIMIZED",
        }
    }

    /// 主 profile 三件套。
    pub fn primary() -> [Self; 3] {
        [
            OptimizationProfile::ZeroRegression,
            OptimizationProfile::FixedFirst,
            OptimizationProfile::Optimized,
        ]
    }

    /// 含 audit profile 的全部 profile。
    pub fn all() -> [Self; 4] {
        [
            OptimizationProfile::EmptyLengthOnly,
            OptimizationProfile::ZeroRegression,
            OptimizationProfile::FixedFirst,
            OptimizationProfile::Optimized,
        ]
    }
}

/// 单条候选的 utility 拆解(reviewer 可解释每个分数的来源)。
#[derive(Clone, Debug, Default)]
pub struct UtilityBreakdown {
    /// 目标词的混合频率质量。
    pub frequency_mass: f64,
    /// baseline 成本拆解(真实当前 rank/fanout)。
    pub baseline: CostBreakdown,
    /// shortcut 成本拆解(projected rank/fanout)。
    pub shortcut: CostBreakdown,
    /// 毛收益:frequency_mass × (baseline − shortcut)。
    pub gross_saving: f64,
    /// 既有候选扰动成本(已含 disruption 系数;FIXED_FIRST 恒为 0)。
    pub disruption_cost: f64,
    /// 净收益:gross − disruption。
    pub net_utility: f64,
}

/// 一条既有候选被扰动的记录(OPTIMIZED)。
#[derive(Clone, Debug)]
pub struct DisruptionRecord {
    /// 所在码。
    pub code: String,
    /// 被扰动候选文本。
    pub text: String,
    /// 来源层。
    pub source: CandidateSource,
    /// 扰动前名次。
    pub old_rank: u32,
    /// 扰动后名次。
    pub new_rank: u32,
    /// 有效成本增量。
    pub delta_cost: f64,
    /// 频率加权成本(混合频率 × delta)。
    pub weighted_cost: f64,
    /// 被扰动候选的混合频率质量。
    pub candidate_mass: f64,
}

/// 一条候选在一个 profile 下的完整评估(含 gate 拒绝情形)。
#[derive(Clone, Debug)]
pub struct CandidateEvaluation {
    /// shortcut 码。
    pub shortcut_code: KeySequence,
    /// 投影模式(如 `FI`)。
    pub mode: String,
    /// 是否可推荐(gate 通过且净收益为正)。
    pub eligible: bool,
    /// gate 拒绝原因(`None` 表示 gate 通过)。
    pub gate_reason: Option<&'static str>,
    /// shortcut 码现有扇出。
    pub existing_fanout: usize,
    /// 碰撞类型。
    pub collision_class: CollisionClass,
    /// baseline 组扇出。
    pub baseline_fanout: usize,
    /// baseline 组内名次。
    pub baseline_rank: u32,
    /// 插入后 projected 名次(gate 拒绝时为 0)。
    pub projected_rank: u32,
    /// 插入后 projected 扇出(gate 拒绝时为 0)。
    pub projected_fanout: usize,
    /// utility 拆解(gate 拒绝时为默认值)。
    pub breakdown: UtilityBreakdown,
    /// 该候选插入导致的扰动记录(OPTIMIZED;其他 profile 为空)。
    pub disruptions: Vec<DisruptionRecord>,
}

/// 一条推荐关系:一个词的一个 shortcut(即被选取的评估)。
#[derive(Clone, Debug)]
pub struct ShortcutAssignment {
    /// 词语。
    pub word: String,
    /// 完整码。
    pub full_code: KeySequence,
    /// 万象聚合频率分数。
    pub frequency_score: u64,
    /// 节省键数。
    pub keys_saved: usize,
    /// 评估明细。
    pub evaluation: CandidateEvaluation,
}

/// 单个 profile 一次运行的聚合统计。
#[derive(Clone, Debug, Default)]
pub struct ProfileStats {
    /// 参与评分的目标数。
    pub targets: usize,
    /// 已评分候选数。
    pub scored_candidates: usize,
    /// 正收益且通过 gate 的候选数。
    pub eligible_candidates: usize,
    /// 推荐分配数。
    pub assigned_words: usize,
    /// 频率加权按键(全部目标,baseline)。
    pub weighted_keys_before: f64,
    /// 频率加权按键(分配后)。
    pub weighted_keys_after: f64,
    /// 平均 shortcut 长度(按分配数均值)。
    pub mean_shortcut_length: f64,
    /// 推荐中 shortcut 码已被现有候选占用的数量。
    pub exact_code_collisions: usize,
    /// 既有 exact top1 变更数(OPTIMIZED)。
    pub top1_changes: usize,
    /// 其中 3 码单字 top1 变更数。
    pub threekey_top1_changes: usize,
    /// 频率加权被置换候选质量合计。
    pub weighted_displaced_mass: f64,
    /// 频率加权扰动成本合计(有效成本增量维度)。
    pub weighted_disruption_cost: f64,
}

impl ProfileStats {
    /// 频率加权节省按键。
    pub fn weighted_keys_saved(&self) -> f64 {
        self.weighted_keys_before - self.weighted_keys_after
    }

    /// 节省百分比。
    pub fn saving_percentage(&self) -> f64 {
        if self.weighted_keys_before <= 0.0 {
            0.0
        } else {
            100.0 * self.weighted_keys_saved() / self.weighted_keys_before
        }
    }
}

/// 一个 profile 的一次完整优化结果。
pub struct OptimizationOutcome {
    /// 运行 profile。
    pub profile: OptimizationProfile,
    /// 推荐分配(按贪心选取顺序 = 收益降序)。
    pub assignments: Vec<ShortcutAssignment>,
    /// 全部扰动记录(OPTIMIZED;其他 profile 为空)。
    pub disruptions: Vec<DisruptionRecord>,
    /// 聚合统计。
    pub stats: ProfileStats,
}

/// 评估单条候选;gate 拒绝时 `eligible = false` 且 `gate_reason` 说明原因。
#[allow(clippy::too_many_arguments)]
pub fn evaluate_candidate(
    target: &WordTarget,
    candidate: &ShortcutCandidate,
    occupancy: &CodeOccupancy,
    frequency: &FrequencyModel,
    scale: &FrequencyScale,
    cost: &CostModel,
    profile: OptimizationProfile,
) -> CandidateEvaluation {
    let code = candidate.shortcut_code();
    let length = code.len();
    let group = occupancy.group(code);
    let existing_fanout = group.map_or(0, |g| g.len());

    // baseline:目标词自身完整码组的真实状态。
    let baseline_group = occupancy
        .group(target.full_code())
        .expect("不变量:词目标完整码组必然存在");
    let baseline_fanout = baseline_group.len();
    let baseline_rank = baseline_group
        .iter()
        .find(|c| c.source() == CandidateSource::FixedWord && c.text() == target.word())
        .unwrap_or_else(|| {
            panic!(
                "不变量:词 {} 必然占用其完整码 {}",
                target.word(),
                target.full_code()
            )
        })
        .rank();

    let mut evaluation = CandidateEvaluation {
        shortcut_code: code.clone(),
        mode: candidate.mode().pattern(),
        eligible: false,
        gate_reason: None,
        existing_fanout,
        collision_class: occupancy.collision_class(code),
        baseline_fanout,
        baseline_rank,
        projected_rank: 0,
        projected_fanout: 0,
        breakdown: UtilityBreakdown::default(),
        disruptions: Vec::new(),
    };

    // profile 门禁与 projected 排名。
    let projected_rank: u32;
    match profile {
        OptimizationProfile::EmptyLengthOnly => {
            if !matches!(length, 5 | 7) {
                evaluation.gate_reason = Some("码长不在 5/7 键空闲层");
                return evaluation;
            }
            if existing_fanout != 0 {
                evaluation.gate_reason = Some("exact code 已占用");
                return evaluation;
            }
            projected_rank = 1;
        }
        OptimizationProfile::ZeroRegression => {
            if existing_fanout != 0 {
                evaluation.gate_reason = Some("exact code 已占用");
                return evaluation;
            }
            projected_rank = 1;
        }
        OptimizationProfile::FixedFirst => {
            projected_rank = u32::try_from(existing_fanout + 1).expect("名次超出 u32");
        }
        OptimizationProfile::Optimized => {
            let group = group.unwrap_or(&[]);
            let has_fullcode_char = length == 4
                && group
                    .iter()
                    .any(|c| c.source() == CandidateSource::CharCode);
            if has_fullcode_char {
                // 硬保护:4 键规范全码 top 不被词语挤掉 —— shortcut 追加到组尾。
                projected_rank = u32::try_from(existing_fanout + 1).expect("名次超出 u32");
            } else {
                // 自由竞争:按混合频率模拟插入位置(同分现有候选优先,保证确定性)。
                let shortcut_weight = frequency.target_weight(scale, target.frequency_score());
                let before = group
                    .iter()
                    .filter(|c| {
                        frequency.candidate_weight(
                            scale,
                            c.source(),
                            c.text(),
                            code,
                            c.frequency_score(),
                        ) >= shortcut_weight
                    })
                    .count();
                projected_rank = u32::try_from(before + 1).expect("名次超出 u32");
                let projected_fanout = existing_fanout + 1;
                for c in group.iter().skip(before) {
                    let old_rank = c.rank();
                    let new_rank = old_rank + 1;
                    let delta = (cost.selection_cost(new_rank)
                        + cost.ambiguity_cost(projected_fanout))
                        - (cost.selection_cost(old_rank) + cost.ambiguity_cost(existing_fanout));
                    if delta <= 0.0 {
                        continue;
                    }
                    let mass = frequency.candidate_weight(
                        scale,
                        c.source(),
                        c.text(),
                        code,
                        c.frequency_score(),
                    );
                    evaluation.disruptions.push(DisruptionRecord {
                        code: code.to_string(),
                        text: c.text().to_string(),
                        source: c.source(),
                        old_rank,
                        new_rank,
                        delta_cost: delta,
                        weighted_cost: mass * delta,
                        candidate_mass: mass,
                    });
                }
            }
        }
    }
    let projected_fanout = existing_fanout + 1;
    evaluation.projected_rank = projected_rank;
    evaluation.projected_fanout = projected_fanout;

    // 成本与收益。
    let baseline = cost.baseline_cost(target.full_code().len(), baseline_rank, baseline_fanout);
    let shortcut = cost.shortcut_cost(length, projected_rank, projected_fanout, candidate.mode());
    let frequency_mass = frequency.target_weight(scale, target.frequency_score());
    let gross_saving = frequency_mass * (baseline.total() - shortcut.total());
    let disruption_cost = cost.disruption_coeff
        * (evaluation
            .disruptions
            .iter()
            .map(|d| d.weighted_cost)
            .sum::<f64>()
            + 0.0);
    let net_utility = gross_saving - disruption_cost;
    evaluation.breakdown = UtilityBreakdown {
        frequency_mass,
        baseline,
        shortcut,
        gross_saving,
        disruption_cost,
        net_utility,
    };
    evaluation.eligible = net_utility > 0.0;
    evaluation
}

/// 评估一个目标的全部候选(哨兵详查用,含被 gate 拒绝的候选)。
#[allow(clippy::too_many_arguments)]
pub fn evaluate_target(
    target: &WordTarget,
    occupancy: &CodeOccupancy,
    frequency: &FrequencyModel,
    scale: &FrequencyScale,
    cost: &CostModel,
    profile: OptimizationProfile,
) -> Vec<CandidateEvaluation> {
    target
        .candidates()
        .iter()
        .map(|candidate| {
            evaluate_candidate(
                target, candidate, occupancy, frequency, scale, cost, profile,
            )
        })
        .collect()
}

/// 聚合频率加权按键:before 全部按完整码计;after 仅对恰好被分配的
/// `(word, full_code)` target 按 shortcut 码长计,其余 target 仍按完整码计。
///
/// target identity 是 `(word, full_code)`:同一 surface word 若未来存在多个
/// 完整码(多音词),只有被 optimizer 选中的那个 target 享受 shortcut 节省。
fn aggregate_weighted_keys(
    targets: &[WordTarget],
    assignments: &[ShortcutAssignment],
    weight_of: impl Fn(&WordTarget) -> f64,
) -> (f64, f64) {
    let assigned_by_target: BTreeMap<(&str, &KeySequence), &ShortcutAssignment> = assignments
        .iter()
        .map(|a| ((a.word.as_str(), &a.full_code), a))
        .collect();
    let mut before = 0.0;
    let mut after = 0.0;
    for target in targets {
        let weight = weight_of(target);
        before += weight * target.full_code().len() as f64;
        let keys = assigned_by_target
            .get(&(target.word(), target.full_code()))
            .map_or(target.full_code().len(), |a| {
                a.evaluation.shortcut_code.len()
            });
        after += weight * keys as f64;
    }
    (before, after)
}

/// 确定性贪心优化:评分 → 全序排序 → word/code 双唯一分配。
pub fn optimize(
    targets: &[WordTarget],
    occupancy: &CodeOccupancy,
    frequency: &FrequencyModel,
    scale: &FrequencyScale,
    cost: &CostModel,
    profile: OptimizationProfile,
) -> OptimizationOutcome {
    let mut scored: Vec<(&WordTarget, CandidateEvaluation)> = Vec::new();
    let mut scored_count = 0usize;
    for target in targets {
        for evaluation in evaluate_target(target, occupancy, frequency, scale, cost, profile) {
            scored_count += 1;
            if evaluation.eligible {
                scored.push((target, evaluation));
            }
        }
    }
    let eligible_count = scored.len();

    // 确定性全序:净收益降序 → 词频降序 → 省键降序 → 码长升序 → 词升序 → 码升序。
    scored.sort_by(|(target_a, a), (target_b, b)| {
        b.breakdown
            .net_utility
            .total_cmp(&a.breakdown.net_utility)
            .then(target_b.frequency_score().cmp(&target_a.frequency_score()))
            .then(
                (target_a.full_code().len() - a.shortcut_code.len())
                    .cmp(&(target_b.full_code().len() - b.shortcut_code.len()))
                    .reverse(),
            )
            .then(a.shortcut_code.len().cmp(&b.shortcut_code.len()))
            .then(target_a.word().cmp(target_b.word()))
            .then(a.shortcut_code.cmp(&b.shortcut_code))
    });

    let mut assigned_words: BTreeSet<String> = BTreeSet::new();
    let mut assigned_codes: BTreeSet<KeySequence> = BTreeSet::new();
    let mut assignments: Vec<ShortcutAssignment> = Vec::new();
    let mut disruptions: Vec<DisruptionRecord> = Vec::new();
    for (target, evaluation) in scored {
        if assigned_words.contains(target.word())
            || assigned_codes.contains(&evaluation.shortcut_code)
        {
            continue;
        }
        assigned_words.insert(target.word().to_string());
        assigned_codes.insert(evaluation.shortcut_code.clone());
        disruptions.extend(evaluation.disruptions.clone());
        assignments.push(ShortcutAssignment {
            word: target.word().to_string(),
            full_code: target.full_code().clone(),
            frequency_score: target.frequency_score(),
            keys_saved: target.full_code().len() - evaluation.shortcut_code.len(),
            evaluation,
        });
    }

    // 聚合统计。
    let mut stats = ProfileStats {
        targets: targets.len(),
        scored_candidates: scored_count,
        eligible_candidates: eligible_count,
        assigned_words: assignments.len(),
        ..ProfileStats::default()
    };
    let (before, after) = aggregate_weighted_keys(targets, &assignments, |t| {
        frequency.target_weight(scale, t.frequency_score())
    });
    stats.weighted_keys_before = before;
    stats.weighted_keys_after = after;
    stats.mean_shortcut_length = if assignments.is_empty() {
        0.0
    } else {
        assignments
            .iter()
            .map(|a| a.evaluation.shortcut_code.len())
            .sum::<usize>() as f64
            / assignments.len() as f64
    };
    stats.exact_code_collisions = assignments
        .iter()
        .filter(|a| a.evaluation.existing_fanout > 0)
        .count();
    // 注:空和归一化为 +0.0(std 空迭代器 f64 sum 产生 -0.0,避免报告出现 -0.0000e0)。
    stats.weighted_disruption_cost = disruptions.iter().map(|d| d.weighted_cost).sum::<f64>() + 0.0;
    stats.weighted_displaced_mass = disruptions.iter().map(|d| d.candidate_mass).sum::<f64>() + 0.0;
    // top1 变更:shortcut 以 rank 1 插入非空组。
    let top1_codes: BTreeSet<String> = assignments
        .iter()
        .filter(|a| a.evaluation.projected_rank == 1 && a.evaluation.existing_fanout > 0)
        .map(|a| a.evaluation.shortcut_code.to_string())
        .collect();
    stats.top1_changes = top1_codes.len();
    stats.threekey_top1_changes = disruptions
        .iter()
        .filter(|d| {
            d.source == CandidateSource::CharCode
                && d.code.len() == 3
                && d.old_rank == 1
                && top1_codes.contains(&d.code)
        })
        .map(|d| d.code.as_str())
        .collect::<BTreeSet<_>>()
        .len();

    OptimizationOutcome {
        profile,
        assignments,
        disruptions,
        stats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_evaluation(shortcut_code: &KeySequence) -> CandidateEvaluation {
        CandidateEvaluation {
            shortcut_code: shortcut_code.clone(),
            mode: "FI".to_string(),
            eligible: true,
            gate_reason: None,
            existing_fanout: 0,
            collision_class: CollisionClass::None,
            baseline_fanout: 1,
            baseline_rank: 1,
            projected_rank: 1,
            projected_fanout: 1,
            breakdown: UtilityBreakdown::default(),
            disruptions: Vec::new(),
        }
    }

    /// 同一 surface word 的两个 `(word, full_code)` target:只有被分配的那个
    /// 享受 shortcut 节省,另一个仍按完整码计费(不被同名 target 株连)。
    #[test]
    fn aggregate_weighted_keys_is_per_word_and_code() {
        let code_a: KeySequence = "uijm".parse().unwrap();
        let code_b: KeySequence = "uijmao".parse().unwrap();
        let targets = vec![
            WordTarget::new_for_test("测试词", code_a.clone(), 100),
            WordTarget::new_for_test("测试词", code_b.clone(), 100),
        ];
        let assignment = ShortcutAssignment {
            word: "测试词".to_string(),
            full_code: code_a.clone(),
            frequency_score: 100,
            keys_saved: 1,
            evaluation: test_evaluation(&"uij".parse().unwrap()),
        };
        let (before, after) = aggregate_weighted_keys(&targets, &[assignment], |_| 1.0);
        assert_eq!(before, 4.0 + 6.0);
        // (测试词, uijm) 用 shortcut uij(3 键);(测试词, uijmao) 仍按完整码 6 键。
        assert_eq!(after, 3.0 + 6.0);
    }
}
