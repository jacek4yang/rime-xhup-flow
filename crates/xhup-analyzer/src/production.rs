//! Production 简码选择:把 ZERO_REGRESSION 分析结果中对 sensitivity 假设高度
//! 稳定的子集固化为 canonical 生产数据。
//!
//! policy `zero-regression-high-v1`(冻结):
//!
//! ```text
//! profile            = ZERO_REGRESSION(baseline fixed exact code 必须空闲)
//! reference run      = balanced × normalized(char_share 0.50, Conservative)
//! robustness         = 30 次 normalized sensitivity 运行中的同码票数
//! gate               = 整数交叉乘法 votes × 5 >= total_runs × 4(即 ≥ 4/5)
//!                      且 robustness 最多票码 == reference assignment 码
//! ```
//!
//! 选择永远基于 [`CodeOccupancy::build_baseline_fixed`],绝不基于
//! current-production occupancy(否则导出会自引用消失)。robustness 阈值用
//! 整数票数比较,不把 deterministic canonical data 建立在浮点阈值边界上。
//!
//! policy 的任何语义变化(threshold、reference run、算法)视为 policy version
//! change + canonical data review,不静默重新生成用户肌肉记忆数据。

use std::collections::BTreeMap;

use xhup_core::KeySequence;

use crate::frequency::{CharCodeUsage, FrequencyScale};
use crate::occupancy::CodeOccupancy;
use crate::optimize::{OptimizationProfile, ShortcutAssignment};
use crate::sweep::{WordRobustness, robustness_map, run_normalized_grid};
use crate::{AnalysisData, CostModel};

/// Production selection policy 的稳定版本标识(写入 canonical TSV 头)。
pub const PRODUCTION_SHORTCUT_POLICY_VERSION: &str = "zero-regression-high-v1";

/// Robustness 阈值分子(4/5)。
pub const ROBUSTNESS_NUMERATOR: usize = 4;

/// Robustness 阈值分母(4/5)。
pub const ROBUSTNESS_DENOMINATOR: usize = 5;

/// Sensitivity 运行数:5 operating points × 3 混合比 × 2 归属假设。
pub const SENSITIVITY_RUNS: usize = 30;

/// reference run 的频率尺度:normalized 50:50,3 码归属保守假设。
pub fn reference_scale() -> FrequencyScale {
    FrequencyScale::Normalized {
        char_share: 0.5,
        usage: CharCodeUsage::Conservative,
    }
}

/// reference run 的成本模型:operating points 中的 balanced 点。
pub fn reference_cost() -> CostModel {
    crate::sweep::operating_points()[2].cost_model()
}

/// 一条 production 简码关系(canonical TSV 的一行)。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionShortcutSelection {
    /// 词语。
    pub word: String,
    /// 完整码(保留;shortcut 是新增别名,不是替换)。
    pub full_code: KeySequence,
    /// shortcut 码(= reference run assignment 码)。
    pub shortcut_code: KeySequence,
    /// F/I 投影模式(如 `FI`)。
    pub mode: String,
    /// 万象聚合频率分数(仅用于 audit 排序,不写入 canonical TSV)。
    pub frequency_score: u64,
    /// robustness 最多票码的选中票数(= 该码在 30 次运行中的选中次数)。
    pub top_code_votes: usize,
    /// sensitivity 总运行数。
    pub total_runs: usize,
}

/// 一条被 production gate 排除的 reference assignment(audit 用)。
#[derive(Clone, Debug)]
pub struct ProductionExclusion {
    /// 词语。
    pub word: String,
    /// reference run 分配的 shortcut 码。
    pub reference_code: KeySequence,
    /// 万象聚合频率分数。
    pub frequency_score: u64,
    /// robustness 最多票码(无记录时为 `None`)。
    pub top_code: Option<String>,
    /// 最多票码的选中票数与总运行数(无记录时为 (0, 0))。
    pub votes: (usize, usize),
    /// 排除原因。
    pub reason: ExclusionReason,
}

/// production gate 的排除原因(selection audit 分类统计)。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ExclusionReason {
    /// reference assignment 存在,但 30 次 normalized 运行中无该词记录。
    NoRobustnessEvidence,
    /// robustness 最多票码与 reference assignment 码不一致。
    TopCodeMismatch,
    /// 同码票数低于 4/5 阈值(整数交叉乘法判定)。
    BelowThreshold,
    /// shortcut 码在 baseline fixed occupancy 中非空(理论上恒为 0)。
    BaselineOccupied,
}

/// selection audit 计数。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SelectionAudit {
    /// reference run(balanced ZERO_REGRESSION)的 assignment 数。
    pub reference_assignments: usize,
    /// 30 次 normalized 运行中有记录的不同词数。
    pub robustness_records: usize,
    /// 进入 production 的条数。
    pub selected: usize,
    /// 各排除原因的条数。
    pub excluded: BTreeMap<ExclusionReason, usize>,
}

impl SelectionAudit {
    /// 被排除的总条数。
    pub fn excluded_total(&self) -> usize {
        self.excluded.values().sum()
    }

    /// 某个原因的排除条数。
    pub fn excluded_by(&self, reason: ExclusionReason) -> usize {
        self.excluded.get(&reason).copied().unwrap_or(0)
    }
}

/// 频率加权按键收益审计(reference 尺度,无量纲优化目标)。
#[derive(Clone, Copy, Debug, Default)]
pub struct BenefitAudit {
    /// 全部目标的频率加权按键(baseline,完整码)。
    pub weighted_keys_before: f64,
    /// 完整 ZERO_REGRESSION reference set 的加权节省。
    pub full_zr_keys_saved: f64,
    /// production HIGH 子集的加权节省。
    pub production_keys_saved: f64,
}

impl BenefitAudit {
    /// production 子集保留完整 ZERO_REGRESSION 收益的比例。
    pub fn retained_ratio(&self) -> f64 {
        if self.full_zr_keys_saved <= 0.0 {
            0.0
        } else {
            self.production_keys_saved / self.full_zr_keys_saved
        }
    }
}

/// production selection 的完整证据:30 次 normalized ZERO_REGRESSION 运行
/// (grid 中的 balanced/50:50/conservative 运行同时即 reference run,不重复
/// 第 31 次优化)。
pub struct ProductionEvidence {
    /// reference run 的 assignments(typed 选取,不依赖报告 label 文本)。
    pub reference_assignments: Vec<ShortcutAssignment>,
    /// ZERO_REGRESSION 30 次 normalized 运行的逐词稳健性。
    pub robustness: BTreeMap<String, WordRobustness>,
}

/// 收集 production 选择证据:只跑 ZERO_REGRESSION 的 normalized 主网格。
///
/// `occupancy` 必须是 baseline fixed occupancy。
pub fn collect_evidence(data: &AnalysisData) -> ProductionEvidence {
    let runs = run_normalized_grid(
        &data.targets,
        &data.occupancy,
        &data.frequency,
        &[OptimizationProfile::ZeroRegression],
    );
    assert_eq!(
        runs.len(),
        SENSITIVITY_RUNS,
        "ZERO_REGRESSION normalized 主网格应恰为 {SENSITIVITY_RUNS} 次运行"
    );
    let reference = runs
        .iter()
        .find(|run| {
            run.point == "balanced"
                && matches!(
                    run.scale,
                    FrequencyScale::Normalized { char_share, usage }
                        if char_share == 0.5 && usage == CharCodeUsage::Conservative
                )
        })
        .expect("不变量:主网格必然包含 balanced/50:50/conservative reference 运行");
    let robustness = robustness_map(&runs, OptimizationProfile::ZeroRegression);
    ProductionEvidence {
        reference_assignments: reference.outcome.assignments.clone(),
        robustness,
    }
}

/// production 选择的完整结果。
pub struct ProductionSelection {
    /// 进入 production 的简码集(canonical 序列化顺序)。
    pub selected: Vec<ProductionShortcutSelection>,
    /// 被排除的 reference assignments(含原因,audit 用)。
    pub exclusions: Vec<ProductionExclusion>,
    /// 计数审计。
    pub audit: SelectionAudit,
}

/// 按冻结 policy 从证据中选择 production 简码集。
///
/// gate 顺序(每条 reference assignment 依次判定,audit 分别计数):
/// robustness 记录存在 → top_code 存在 → top_code == reference 码 →
/// 整数票数 ≥ 4/5 → baseline exact fanout == 0。
pub fn select_production_shortcuts(
    evidence: &ProductionEvidence,
    baseline: &CodeOccupancy,
) -> ProductionSelection {
    let mut selected = Vec::new();
    let mut exclusions = Vec::new();
    let mut audit = SelectionAudit {
        reference_assignments: evidence.reference_assignments.len(),
        robustness_records: evidence.robustness.len(),
        ..SelectionAudit::default()
    };
    for assignment in &evidence.reference_assignments {
        let reference_code = assignment.evaluation.shortcut_code.to_string();
        let record = evidence.robustness.get(&assignment.word);
        let top = record.and_then(|r| r.top_code());
        let votes = top.map(|(_, v)| v).unwrap_or(0);
        let total_runs = record.map_or(0, |r| r.total_runs);
        let reason = if record.is_none() || top.is_none() {
            Some(ExclusionReason::NoRobustnessEvidence)
        } else if top.is_some_and(|(code, _)| code != reference_code) {
            Some(ExclusionReason::TopCodeMismatch)
        } else if votes * ROBUSTNESS_DENOMINATOR < total_runs * ROBUSTNESS_NUMERATOR {
            // 整数交叉乘法:votes/total >= 4/5 ⇔ votes×5 >= total×4。
            Some(ExclusionReason::BelowThreshold)
        } else if baseline.fanout(&assignment.evaluation.shortcut_code) != 0 {
            Some(ExclusionReason::BaselineOccupied)
        } else {
            None
        };
        if let Some(reason) = reason {
            *audit.excluded.entry(reason).or_default() += 1;
            exclusions.push(ProductionExclusion {
                word: assignment.word.clone(),
                reference_code: assignment.evaluation.shortcut_code.clone(),
                frequency_score: assignment.frequency_score,
                top_code: top.map(|(code, _)| code.to_string()),
                votes: (votes, total_runs),
                reason,
            });
            continue;
        }
        selected.push(ProductionShortcutSelection {
            word: assignment.word.clone(),
            full_code: assignment.full_code.clone(),
            shortcut_code: assignment.evaluation.shortcut_code.clone(),
            mode: assignment.evaluation.mode.clone(),
            frequency_score: assignment.frequency_score,
            top_code_votes: votes,
            total_runs,
        });
    }
    audit.selected = selected.len();
    // 双唯一不变量(reference run 已保证,production set 再硬断言一次)。
    let mut words = std::collections::BTreeSet::new();
    let mut codes = std::collections::BTreeSet::new();
    for entry in &selected {
        assert!(
            words.insert(&entry.word),
            "production 词重复: {}",
            entry.word
        );
        assert!(
            codes.insert(&entry.shortcut_code),
            "production 码重复: {}",
            entry.shortcut_code
        );
    }
    // canonical 序列化顺序:shortcut 长度 → 码 → 词 → 完整码 → 模式。
    selected.sort_by(|a, b| {
        a.shortcut_code
            .len()
            .cmp(&b.shortcut_code.len())
            .then(a.shortcut_code.cmp(&b.shortcut_code))
            .then(a.word.cmp(&b.word))
            .then(a.full_code.cmp(&b.full_code))
            .then(a.mode.cmp(&b.mode))
    });
    ProductionSelection {
        selected,
        exclusions,
        audit,
    }
}

/// 计算频率加权按键收益审计(reference 尺度,无量纲)。
pub fn benefit_audit(
    data: &AnalysisData,
    evidence: &ProductionEvidence,
    selection: &ProductionSelection,
) -> BenefitAudit {
    let scale = reference_scale();
    let weight = |score: u64| data.frequency.target_weight(&scale, score);
    let before: f64 = data
        .targets
        .iter()
        .map(|t| weight(t.frequency_score()) * t.full_code().len() as f64)
        .sum();
    let full_saved: f64 = evidence
        .reference_assignments
        .iter()
        .map(|a| weight(a.frequency_score) * a.keys_saved as f64)
        .sum();
    let production_saved: f64 = selection
        .selected
        .iter()
        .map(|e| weight(e.frequency_score) * (e.full_code.len() - e.shortcut_code.len()) as f64)
        .sum();
    BenefitAudit {
        weighted_keys_before: before,
        full_zr_keys_saved: full_saved,
        production_keys_saved: production_saved,
    }
}

/// 序列化为 canonical TSV:`词<TAB>完整码<TAB>shortcut 码<TAB>模式`。
///
/// UTF-8、LF、无 BOM、恰好一个末尾换行;`#` 头记录 policy 与 provenance,
/// 不含时间戳、主机、路径等易变内容。utility、浮点分数、名次等 analysis
/// evidence 不属于 production semantic identity,一律不写入。
pub fn serialize_canonical_tsv(selected: &[ProductionShortcutSelection]) -> String {
    let mut out = String::new();
    out.push_str("# XHUP Flow high-robustness ZERO_REGRESSION word shortcuts.\n");
    out.push_str("# Source universe: data/words/wanxiang_base_words.tsv\n");
    out.push_str("# Selection: ZERO_REGRESSION / balanced / normalized 50:50 conservative\n");
    out.push_str(
        "# Robustness gate: same-code stability >= 4/5 over 30 normalized sensitivity runs\n",
    );
    out.push_str("# policy: ");
    out.push_str(PRODUCTION_SHORTCUT_POLICY_VERSION);
    out.push('\n');
    for entry in selected {
        out.push_str(&entry.word);
        out.push('\t');
        out.push_str(&entry.full_code.to_string());
        out.push('\t');
        out.push_str(&entry.shortcut_code.to_string());
        out.push('\t');
        out.push_str(&entry.mode);
        out.push('\n');
    }
    out
}
