//! XHUP Flow 数据分析:编码空间占用、词语简码候选枚举、成本建模、频率收益与
//! 重码成本分析、多 profile 确定性优化模拟与结果报告。
//!
//! 本 crate 是**纯分析工具**:只消费 `xhup-core` / `xhup-generator` 的只读
//! 投影,不改变任何 production 生成行为;分析产物(TSV 转储、报告)不进入
//! 码表。优化器是 deterministic heuristic,不声称数学全局最优;成本模型的
//! 数值是无量纲优化目标,不是真实耗时预测。
//!
//! 词语 shortcut 的 frozen rule:每字只能选择 F(完整双拼两键)或 I(双拼
//! 首键),保持字序,长度 ≥ 3 且不等于完整码。
#![forbid(unsafe_code)]

pub mod candidates;
pub mod cost;
pub mod frequency;
pub mod occupancy;
pub mod optimize;
pub mod report;
pub mod sweep;

pub use candidates::{
    EnumerationStats, Mode, ShortcutCandidate, ShortcutMode, WordTarget, enumerate_targets,
};
pub use cost::{CostBreakdown, CostModel};
pub use frequency::{CharCodeUsage, FrequencyModel, FrequencyScale};
pub use occupancy::{
    CandidateSource, CodeOccupancy, CollisionClass, ExistingCandidate, LayerAudit, LengthStats,
};
pub use optimize::{
    CandidateEvaluation, DisruptionRecord, OptimizationOutcome, OptimizationProfile, ProfileStats,
    ShortcutAssignment, UtilityBreakdown, evaluate_candidate, evaluate_target, optimize,
};
pub use report::{Timings, render_report};
pub use sweep::{
    OperatingPoint, Robustness, SweepRun, WordRobustness, classify, mixtures, operating_points,
    robustness_map, run_sweep,
};
pub use xhup_generator::{CharCodeAnalysisEntry, WordCodeAnalysisEntry};

/// 一次性构建的不可变分析输入(occupancy / 候选 / 频率模型只构建一次,
/// 全部 sweep 运行复用)。
pub struct AnalysisData {
    /// 单字分析证据投影(2/3/4 码固定层)。
    pub chars: Vec<CharCodeAnalysisEntry>,
    /// 词语分析证据投影(100k production 词)。
    pub words: Vec<WordCodeAnalysisEntry>,
    /// 当前固定层 exact-code 占用。
    pub occupancy: CodeOccupancy,
    /// 全部分析目标(identity = (词, 完整码))及其 shortcut 候选。
    pub targets: Vec<WordTarget>,
    /// 候选枚举统计。
    pub enumeration: EnumerationStats,
    /// 频率模型(domain 归一化状态)。
    pub frequency: FrequencyModel,
}

/// 从 generator 只读投影构建全部分析输入。
pub fn build_analysis() -> AnalysisData {
    let chars = xhup_generator::char_code_analysis_entries();
    let words = xhup_generator::word_code_analysis_entries();
    let occupancy = CodeOccupancy::build();
    let (targets, enumeration) = enumerate_targets(&words);
    let frequency = FrequencyModel::build(&chars, &words);
    AnalysisData {
        chars,
        words,
        occupancy,
        targets,
        enumeration,
        frequency,
    }
}
