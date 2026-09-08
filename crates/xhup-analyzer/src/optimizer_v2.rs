//! Optimizer v2 核心:候选位资源建模 + 集中可配置成本/证据参数。
//!
//! 设计文档:docs/optimizer-v2.md。与 v1(optimize.rs)的关系:v1 的
//! profile/sweep/production 管线保留;本模块提供 v2 的**评估内核** —
//! 效用函数消费多信号词汇证据(LexicalEvidence),成本模型把候选位
//! (rank)当作资源,偏离 XHUP 传统的代价显式参数化。
//!
//! 原则:
//!
//! - 每个重要信号显式、可测、缺失显式(None 绝不静默当 0);
//! - 参数集中在一个结构体,扫描框架直接采样;
//! - 效用计算纯函数化,可解释(UtilityBreakdownV2 逐项可查);
//! - 确定性:同输入同参数 → 同输出。

use crate::evidence::LexicalEvidence;

/// 候选位资源:码 + 排名位。
///
/// 码位不是空闲/占用二值:占用已有码的 rank 2 与占新码的 rank 1 是
/// 不同的资源消耗。两个质量项语义不同、必须分开(2026-10 高频词简码
/// 丢失根因:混用导致「没挤任何人也要付扰动费」的过度保守):
///
/// - `occupant_mass`:码内**全部**既有占用质量(竞争强度),放大选择成本;
/// - `displaced_mass`:实际被挤到更靠后位次的占用质量(位次 ≥ 本槽位
///   rank 的候选),只有它进入扰动成本(docs §1:扰动 = 被挤后排的既有
///   候选的效用损失)。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CandidateSlot {
    /// 键数(码长)。
    pub key_len: usize,
    /// 目标候选位(1 = 首选)。
    pub rank: usize,
    /// 码内全部既有占用质量(竞争强度;0 = 空码)。
    pub occupant_mass: f64,
    /// 被本分配实际挤后的占用质量(0 = 无人被挤)。
    pub displaced_mass: f64,
}

/// 成本模型 v2:全部参数集中、可扫描(docs/optimizer-v2.md §2)。
///
/// 数值是无量纲优化目标,不是真实耗时预测。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CostModelV2 {
    /// 每键基础成本。
    pub key_cost: f64,
    /// 候选选择成本(索引 = rank-1;rank 超出用末元素)。
    pub rank_cost: [f64; 4],
    /// 歧义系数:同码既有占用质量对选择成本的放大。
    pub ambiguity_coeff: f64,
    /// 扰动系数:把既有候选挤后的代价。
    pub disruption_coeff: f64,
    /// XHUP 偏离系数:与传统参考映射不一致的代价(prior 得分 0..1 时
    /// 代价 = coeff × (1 - prior))。
    pub xhup_deviation_coeff: f64,
    /// 认知复杂度系数:码与该词既有模式不一致(简码键不是全码前缀)
    /// 的惩罚。
    pub cognitive_complexity_coeff: f64,
    /// 长尾污染系数:低频词占稀缺位的惩罚(按归一化频率阈值计算)。
    pub rare_pollution_coeff: f64,
}

impl Default for CostModelV2 {
    /// 默认工作点(初值来自 v1 网格的稳健区,必须由扫描重新确认 —
    /// 这些值不是结论,只是起点)。
    fn default() -> Self {
        CostModelV2 {
            key_cost: 1.0,
            rank_cost: [0.0, 0.5, 1.0, 2.0],
            ambiguity_coeff: 0.5,
            disruption_coeff: 1.0,
            xhup_deviation_coeff: 1.0,
            cognitive_complexity_coeff: 0.5,
            rare_pollution_coeff: 1.0,
        }
    }
}

impl CostModelV2 {
    /// 指定候选位的选择成本(含歧义放大)。
    pub fn selection_cost(&self, slot: &CandidateSlot) -> f64 {
        let base = self.rank_cost[slot.rank.saturating_sub(1).min(self.rank_cost.len() - 1)];
        base + self.ambiguity_coeff * slot.occupant_mass
    }

    /// 占用该候选位的扰动成本(只计实际被挤后的候选)。
    pub fn disruption_cost(&self, slot: &CandidateSlot) -> f64 {
        self.disruption_coeff * slot.displaced_mass
    }

    /// 击键成本。
    pub fn keystroke_cost(&self, slot: &CandidateSlot) -> f64 {
        slot.key_len as f64 * self.key_cost
    }
}

/// 证据权重:各信号在效用函数中的份额(docs/optimizer-v2.md §2)。
///
/// 缺失信号的规则:效用计算时把缺失信号的权重按已测量信号**重归一化**
/// (绝不把 None 当 0 计入);有效权重由 [`EvidenceWeights::effective`] 给出。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EvidenceWeights {
    /// 全局频率(万象归一化)份额。
    pub global_share: f64,
    /// 会话域频率份额。
    pub conversation_share: f64,
    /// 句子覆盖度份额。
    pub sentence_coverage_weight: f64,
    /// 上下文多样性份额。
    pub context_diversity_weight: f64,
}

impl Default for EvidenceWeights {
    fn default() -> Self {
        EvidenceWeights {
            global_share: 0.5,
            conversation_share: 0.2,
            sentence_coverage_weight: 0.2,
            context_diversity_weight: 0.1,
        }
    }
}

impl EvidenceWeights {
    /// 指定证据条目的有效权重(缺失信号份额重归一化到已测量信号)。
    ///
    /// 返回 (global, conversation, sentence_coverage, context_diversity),
    /// 总和恒为 1(全缺失时退化为 global=1,保证总能评估)。
    pub fn effective(&self, evidence: &LexicalEvidence) -> (f64, f64, f64, f64) {
        let mut weights = [
            (self.global_share, true), // 全局频率恒有(万象,canonical 前提)
            (
                self.conversation_share,
                evidence.conversation_frequency().is_some(),
            ),
            (
                self.sentence_coverage_weight,
                evidence.sentence_coverage().is_some(),
            ),
            (
                self.context_diversity_weight,
                evidence.context_diversity().is_some(),
            ),
        ];
        let present: f64 = weights
            .iter()
            .filter(|(_, available)| *available)
            .map(|(w, _)| *w)
            .sum();
        let scale = if present > 0.0 { 1.0 / present } else { 0.0 };
        for (w, available) in &mut weights {
            if !*available {
                *w = 0.0;
            } else {
                *w *= scale;
            }
        }
        if present == 0.0 {
            (1.0, 0.0, 0.0, 0.0)
        } else {
            (weights[0].0, weights[1].0, weights[2].0, weights[3].0)
        }
    }
}

/// 效用分解:每一项显式可查(可解释性门禁)。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UtilityBreakdownV2 {
    /// 频率效用(多信号加权和)。
    pub frequency_utility: f64,
    /// 节省键数(相对全码)。
    pub keystrokes_saved: f64,
    /// XHUP 风格先验得分(0..1;无参考映射时 0.5 中立)。
    pub xhup_prior: f64,
    /// 选择成本(含歧义)。
    pub selection_cost: f64,
    /// 扰动成本。
    pub disruption_cost: f64,
    /// 击键成本。
    pub keystroke_cost: f64,
    /// 长尾污染惩罚。
    pub rare_pollution: f64,
}

impl UtilityBreakdownV2 {
    /// 总效用(频率收益 + 省键 − 各项成本)。
    pub fn total(&self) -> f64 {
        self.frequency_utility + self.keystrokes_saved + self.xhup_prior
            - self.selection_cost
            - self.disruption_cost
            - self.keystroke_cost
            - self.rare_pollution
    }
}

/// 评估「词 W 占用候选位 slot」的效用(纯函数,可解释)。
///
/// - `evidence`:词的多信号证据;
/// - `prior`:XHUP 风格先验得分 0..=1(无参考映射数据时传 0.5 中立);
/// - `full_code_len`:该词全码键数(省键基准);
/// - `pattern_consistent`:简码键是否为全码前缀(认知/可记忆性)。
pub fn evaluate_assignment(
    evidence: &LexicalEvidence,
    slot: &CandidateSlot,
    cost: &CostModelV2,
    weights: &EvidenceWeights,
    prior: f64,
    full_code_len: usize,
    pattern_consistent: bool,
) -> UtilityBreakdownV2 {
    let (wg, wc, wsc, wcd) = weights.effective(evidence);
    // 各信号统一到可比尺度:频率类信号取 log1p 压缩动态范围。
    let global = (evidence.normalized_frequency()).ln_1p();
    let conversation = evidence
        .conversation_frequency()
        .map(f64::ln_1p)
        .unwrap_or(0.0);
    let sentence = evidence.sentence_coverage().map(f64::ln_1p).unwrap_or(0.0);
    let diversity = evidence
        .context_diversity()
        .map(|d| (d as f64).ln_1p())
        .unwrap_or(0.0);
    let frequency_utility = wg * global + wc * conversation + wsc * sentence + wcd * diversity;

    let keystrokes_saved = full_code_len.saturating_sub(slot.key_len) as f64 * cost.key_cost;
    let prior_penalty = cost.xhup_deviation_coeff * (1.0 - prior);
    let cognitive_penalty = if pattern_consistent {
        0.0
    } else {
        cost.cognitive_complexity_coeff
    };
    // 长尾污染:归一化频率低于中位数量级的词占用稀缺短位(≤3 键)时惩罚。
    let rare_pollution = if slot.key_len <= 3 && evidence.normalized_frequency() < 1e-6 {
        cost.rare_pollution_coeff
    } else {
        0.0
    };

    UtilityBreakdownV2 {
        frequency_utility,
        keystrokes_saved,
        xhup_prior: prior - prior_penalty - cognitive_penalty,
        selection_cost: cost.selection_cost(slot),
        disruption_cost: cost.disruption_cost(slot),
        keystroke_cost: cost.keystroke_cost(slot),
        rare_pollution,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::LexicalEvidenceSet;

    fn evidence_of() -> (crate::AnalysisData, LexicalEvidenceSet) {
        let data = crate::build_analysis();
        let set = LexicalEvidenceSet::build(&data.words, &data.frequency);
        (data, set)
    }

    #[test]
    fn effective_weights_renormalize_over_missing_signals() {
        let (_, set) = evidence_of();
        let all_missing = set
            .entries()
            .iter()
            .find(|e| e.sentence_coverage().is_none())
            .expect("存在未见语料的词");
        let weights = EvidenceWeights::default();
        let (wg, wc, wsc, wcd) = weights.effective(all_missing);
        assert_eq!(wc + wsc + wcd, 0.0, "缺失信号权重归零");
        assert!((wg - 1.0).abs() < 1e-12, "全部缺失时退化为全局频率");
        let observed = set
            .entries()
            .iter()
            .find(|e| e.sentence_coverage().is_some())
            .expect("存在语料覆盖的词");
        let (wg, wc, wsc, wcd) = weights.effective(observed);
        let total = wg + wc + wsc + wcd;
        assert!((total - 1.0).abs() < 1e-12, "有效权重归一化,实际 {total}");
        assert!(wg < 1.0 && wc > 0.0, "已测量信号获得份额");
    }

    #[test]
    fn shorter_slot_with_low_rank_beats_full_code_for_common_word() {
        // 高频词 3 键首选应显著优于 4 键全码(rank 视占用)。
        let (_, set) = evidence_of();
        let women = set.entries().iter().find(|e| e.word() == "我们").unwrap();
        let cost = CostModelV2::default();
        let weights = EvidenceWeights::default();
        let shortcut = evaluate_assignment(
            women,
            &CandidateSlot {
                key_len: 3,
                rank: 1,
                occupant_mass: 0.0,
                displaced_mass: 0.0,
            },
            &cost,
            &weights,
            0.5,
            4,
            true,
        );
        let full = evaluate_assignment(
            women,
            &CandidateSlot {
                key_len: 4,
                rank: 1,
                occupant_mass: 0.0,
                displaced_mass: 0.0,
            },
            &cost,
            &weights,
            0.5,
            4,
            true,
        );
        assert!(
            shortcut.total() > full.total(),
            "简码应优于全码: {} vs {}",
            shortcut.total(),
            full.total()
        );
        assert_eq!(shortcut.keystrokes_saved, 1.0);
        assert_eq!(full.keystrokes_saved, 0.0);
    }

    #[test]
    fn rare_word_on_scarce_slot_is_penalized() {
        // 长尾词占 2 键稀缺位 → rare_pollution 生效。
        // 阈值标定(2026-09 真实分布):normalized 中位 ≈2.6e-6、P25 ≈2.0e-6,
        // 1e-6 ≈ 最底五分位;「木寨」(万象分数 3,归一化 ≈5e-9)是真长尾。
        let (_, set) = evidence_of();
        let rare = set
            .entries()
            .iter()
            .find(|e| e.word() == "木寨")
            .expect("木寨 应在库");
        let cost = CostModelV2::default();
        let breakdown = evaluate_assignment(
            rare,
            &CandidateSlot {
                key_len: 2,
                rank: 1,
                occupant_mass: 0.0,
                displaced_mass: 0.0,
            },
            &cost,
            &EvidenceWeights::default(),
            0.5,
            8,
            true,
        );
        assert!(
            breakdown.rare_pollution > 0.0,
            "低频词占稀缺位应被惩罚: {:?}",
            breakdown
        );
    }

    #[test]
    fn disruption_scales_with_displaced_mass() {
        // 扰动只计被挤后的候选(displaced);竞争强度(occupant)放大选择成本。
        let cost = CostModelV2::default();
        let empty = CandidateSlot {
            key_len: 3,
            rank: 2,
            occupant_mass: 0.0,
            displaced_mass: 0.0,
        };
        let tailgated = CandidateSlot {
            key_len: 3,
            rank: 2,
            occupant_mass: 0.7,
            displaced_mass: 0.0,
        };
        let displacing = CandidateSlot {
            key_len: 3,
            rank: 1,
            occupant_mass: 0.7,
            displaced_mass: 0.7,
        };
        assert_eq!(cost.disruption_cost(&empty), 0.0);
        assert_eq!(
            cost.disruption_cost(&tailgated),
            0.0,
            "跟排在占用者之后不挤人,扰动为零"
        );
        assert!(cost.disruption_cost(&displacing) > 0.0);
        assert!(
            cost.selection_cost(&tailgated) > cost.selection_cost(&empty),
            "占用质量应放大选择成本"
        );
    }

    #[test]
    fn evaluation_is_deterministic() {
        let (_, set) = evidence_of();
        let women = set.entries().iter().find(|e| e.word() == "我们").unwrap();
        let slot = CandidateSlot {
            key_len: 2,
            rank: 1,
            occupant_mass: 0.3,
            displaced_mass: 0.0,
        };
        let a = evaluate_assignment(
            women,
            &slot,
            &CostModelV2::default(),
            &EvidenceWeights::default(),
            0.8,
            4,
            true,
        );
        let b = evaluate_assignment(
            women,
            &slot,
            &CostModelV2::default(),
            &EvidenceWeights::default(),
            0.8,
            4,
            true,
        );
        assert_eq!(a, b);
    }
}
