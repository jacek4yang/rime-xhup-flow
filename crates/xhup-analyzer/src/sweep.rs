//! 敏感性扫描与稳健性聚合。
//!
//! 主网格:5 个 operating points × 3 个 char:word 混合比 × 2 个 3 码单字归属
//! 假设 = 每个 profile 30 次运行;另加 raw-score 诊断运行(不进入稳健性统计)。
//! 所有运行复用同一份不可变的 occupancy/candidate/frequency 数据,只重新评分
//! 与分配。本模块不声称任何"最优参数",只呈现 Pareto/operating point 面。

use std::collections::BTreeMap;

use crate::candidates::WordTarget;
use crate::cost::CostModel;
use crate::frequency::{CharCodeUsage, FrequencyModel, FrequencyScale};
use crate::occupancy::CodeOccupancy;
use crate::optimize::{OptimizationOutcome, OptimizationProfile, optimize};

/// operating point 的稳定 typed identity。
///
/// production policy(`zero-regression-high-v1`)的 reference run 以此为准,
/// 不依赖展示字符串或数组位置;展示文字通过 [`OperatingPointId::label`] 获得。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum OperatingPointId {
    /// 极保守(rank/歧义/扰动成本最高,模式切换有惩罚)。
    VeryConservative,
    /// 保守。
    Conservative,
    /// 均衡(production reference 运行点)。
    Balanced,
    /// 激进。
    Aggressive,
    /// 极激进。
    VeryAggressive,
}

impl OperatingPointId {
    /// 全部 identity(very conservative → very aggressive)。
    pub const ALL: [OperatingPointId; 5] = [
        OperatingPointId::VeryConservative,
        OperatingPointId::Conservative,
        OperatingPointId::Balanced,
        OperatingPointId::Aggressive,
        OperatingPointId::VeryAggressive,
    ];

    /// 报告用稳定标签。
    pub fn label(self) -> &'static str {
        match self {
            OperatingPointId::VeryConservative => "very_conservative",
            OperatingPointId::Conservative => "conservative",
            OperatingPointId::Balanced => "balanced",
            OperatingPointId::Aggressive => "aggressive",
            OperatingPointId::VeryAggressive => "very_aggressive",
        }
    }

    /// 该 identity 对应的成本参数组合(typed,无数组位置依赖)。
    pub fn operating_point(self) -> OperatingPoint {
        let id = self;
        match self {
            OperatingPointId::VeryConservative => OperatingPoint {
                id,
                selection_rank2_9: 2.0,
                selection_rank10_plus: 4.0,
                ambiguity_coeff: 1.0,
                disruption_coeff: 4.0,
                mode_complexity_per_transition: 0.1,
            },
            OperatingPointId::Conservative => OperatingPoint {
                id,
                selection_rank2_9: 1.5,
                selection_rank10_plus: 3.0,
                ambiguity_coeff: 0.75,
                disruption_coeff: 2.0,
                mode_complexity_per_transition: 0.05,
            },
            OperatingPointId::Balanced => OperatingPoint {
                id,
                selection_rank2_9: 1.0,
                selection_rank10_plus: 2.0,
                ambiguity_coeff: 0.5,
                disruption_coeff: 1.0,
                mode_complexity_per_transition: 0.0,
            },
            OperatingPointId::Aggressive => OperatingPoint {
                id,
                selection_rank2_9: 0.5,
                selection_rank10_plus: 1.0,
                ambiguity_coeff: 0.25,
                disruption_coeff: 0.5,
                mode_complexity_per_transition: 0.0,
            },
            OperatingPointId::VeryAggressive => OperatingPoint {
                id,
                selection_rank2_9: 0.25,
                selection_rank10_plus: 0.5,
                ambiguity_coeff: 0.1,
                disruption_coeff: 0.25,
                mode_complexity_per_transition: 0.0,
            },
        }
    }
}

/// 一个代表性 operating point(成本参数组合)。
#[derive(Clone, Copy, Debug)]
pub struct OperatingPoint {
    /// 稳定 typed identity(展示文字:`id.label()`)。
    pub id: OperatingPointId,
    /// rank 2..=9 选择成本。
    pub selection_rank2_9: f64,
    /// rank ≥ 10 选择成本。
    pub selection_rank10_plus: f64,
    /// 歧义成本系数。
    pub ambiguity_coeff: f64,
    /// 扰动成本系数。
    pub disruption_coeff: f64,
    /// 每次 F/I 切换的复杂度惩罚。
    pub mode_complexity_per_transition: f64,
}

impl OperatingPoint {
    /// 展开为成本模型。
    pub fn cost_model(&self) -> CostModel {
        CostModel {
            selection_rank1: 0.0,
            selection_rank2_9: self.selection_rank2_9,
            selection_rank10_plus: self.selection_rank10_plus,
            ambiguity_coeff: self.ambiguity_coeff,
            disruption_coeff: self.disruption_coeff,
            mode_complexity_per_transition: self.mode_complexity_per_transition,
        }
    }
}

/// 五个代表性 operating points(very conservative → very aggressive)。
pub fn operating_points() -> [OperatingPoint; 5] {
    OperatingPointId::ALL.map(OperatingPointId::operating_point)
}

/// char:word 混合比(char 侧占比)。
pub fn mixtures() -> [(&'static str, f64); 3] {
    [("25:75", 0.25), ("50:50", 0.5), ("75:25", 0.75)]
}

/// 一次 sweep 运行。
pub struct SweepRun {
    /// 运行标签(profile / operating point / 频率尺度)。
    pub label: String,
    /// profile。
    pub profile: OptimizationProfile,
    /// operating point 的稳定 typed identity。
    pub point: OperatingPointId,
    /// 频率尺度。
    pub scale: FrequencyScale,
    /// 是否 raw-score 诊断运行(不进入稳健性统计)。
    pub diagnostic: bool,
    /// 优化结果。
    pub outcome: OptimizationOutcome,
}

/// 执行 normalized 主网格(不含 raw 诊断运行)。
///
/// production selection 的快速路径复用本函数:只跑 ZERO_REGRESSION 的
/// 30 次 normalized 运行,结果与完整 sweep 中对应数据严格一致。
pub fn run_normalized_grid(
    targets: &[WordTarget],
    occupancy: &CodeOccupancy,
    frequency: &FrequencyModel,
    profiles: &[OptimizationProfile],
) -> Vec<SweepRun> {
    let points = operating_points();
    let mut runs = Vec::new();
    for &profile in profiles {
        for point in &points {
            let cost = point.cost_model();
            for (mixture_label, char_share) in mixtures() {
                for usage in [CharCodeUsage::Conservative, CharCodeUsage::Split] {
                    let scale = FrequencyScale::Normalized { char_share, usage };
                    let outcome = optimize(targets, occupancy, frequency, &scale, &cost, profile);
                    runs.push(SweepRun {
                        label: format!(
                            "{} | {} | {} | {}",
                            profile.label(),
                            point.id.label(),
                            mixture_label,
                            usage.label()
                        ),
                        profile,
                        point: point.id,
                        scale,
                        diagnostic: false,
                        outcome,
                    });
                }
            }
        }
    }
    runs
}

/// 执行完整 sweep:主网格 + raw 诊断。
pub fn run_sweep(
    targets: &[WordTarget],
    occupancy: &CodeOccupancy,
    frequency: &FrequencyModel,
    profiles: &[OptimizationProfile],
) -> Vec<SweepRun> {
    let mut runs = run_normalized_grid(targets, occupancy, frequency, profiles);
    for &profile in profiles {
        // raw-score 诊断:balanced 点 × 3 混合比(raw 不使用混合比,但频率聚合
        // 仍按目标全量;为对齐 grid 结构仅跑 balanced 一次即可说明尺度风险)。
        let balanced = OperatingPointId::Balanced.operating_point();
        let scale = FrequencyScale::RawDiagnostic;
        let outcome = optimize(
            targets,
            occupancy,
            frequency,
            &scale,
            &balanced.cost_model(),
            profile,
        );
        runs.push(SweepRun {
            label: format!(
                "{} | {} | raw-diagnostic",
                profile.label(),
                balanced.id.label()
            ),
            profile,
            point: balanced.id,
            scale,
            diagnostic: true,
            outcome,
        });
    }
    runs
}

/// 一个词的稳健性记录。
#[derive(Clone, Debug, Default)]
pub struct WordRobustness {
    /// 总运行数(该 profile 的非诊断运行)。
    pub total_runs: usize,
    /// 该词被分配任意 shortcut 的运行数。
    pub selected_runs: usize,
    /// 各 shortcut 码的选中次数。
    pub code_votes: BTreeMap<String, usize>,
}

impl WordRobustness {
    /// 最多票数的 shortcut 码及其选中次数。
    pub fn top_code(&self) -> Option<(&str, usize)> {
        self.code_votes
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
            .map(|(code, votes)| (code.as_str(), *votes))
    }

    /// 选中率:被分配任意 shortcut 的运行比例。
    pub fn selection_rate(&self) -> f64 {
        if self.total_runs == 0 {
            0.0
        } else {
            self.selected_runs as f64 / self.total_runs as f64
        }
    }

    /// 同码稳定度:得票最多码的选中比例(稳健性分级以此为准)。
    pub fn same_code_stability(&self) -> f64 {
        if self.total_runs == 0 {
            0.0
        } else {
            self.top_code()
                .map_or(0.0, |(_, votes)| votes as f64 / self.total_runs as f64)
        }
    }
}

/// 稳健性分级。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Robustness {
    /// 同码稳定度 ≥ 80%。
    High,
    /// 同码稳定度 ≥ 40%。
    Medium,
    /// 同码稳定度 < 40%。
    Low,
}

impl Robustness {
    /// 报告用稳定标签。
    pub fn label(self) -> &'static str {
        match self {
            Robustness::High => "HIGH",
            Robustness::Medium => "MEDIUM",
            Robustness::Low => "LOW",
        }
    }
}

/// 按同码稳定度分级。
pub fn classify(stability: f64) -> Robustness {
    if stability >= 0.8 {
        Robustness::High
    } else if stability >= 0.4 {
        Robustness::Medium
    } else {
        Robustness::Low
    }
}

/// 聚合某个 profile 全部非诊断运行的稳健性(词 → 记录)。
pub fn robustness_map(
    runs: &[SweepRun],
    profile: OptimizationProfile,
) -> BTreeMap<String, WordRobustness> {
    let mut map: BTreeMap<String, WordRobustness> = BTreeMap::new();
    let relevant: Vec<&SweepRun> = runs
        .iter()
        .filter(|run| run.profile == profile && !run.diagnostic)
        .collect();
    for run in &relevant {
        let mut seen_words: BTreeMap<String, String> = BTreeMap::new();
        for assignment in &run.outcome.assignments {
            seen_words.insert(
                assignment.word.clone(),
                assignment.evaluation.shortcut_code.to_string(),
            );
        }
        for (word, code) in seen_words {
            let record = map.entry(word).or_default();
            record.selected_runs += 1;
            *record.code_votes.entry(code).or_default() += 1;
        }
    }
    let total = relevant.len();
    for record in map.values_mut() {
        record.total_runs = total;
    }
    map
}
