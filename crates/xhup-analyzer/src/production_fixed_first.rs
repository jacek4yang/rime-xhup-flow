//! Production FIXED_FIRST 简码选择:在 ZERO_REGRESSION 层(PR #22)之上,
//! 把「与 baseline fixed exact code 重码、但分析表明仍值得使用」的高稳健
//! 词语简码固化为第二层 canonical 生产数据。
//!
//! policy `fixed-first-high-v1`(冻结):
//!
//! ```text
//! profile            = FIXED_FIRST(existing fixed 候选次序绝对不变,
//!                      shortcut 追加到组尾,rank = baseline_fanout + 1)
//! candidate grammar  = MonotoneSuffixInitialsV2(单调后缀缩写 F* I*,
//!                      至少一个 I;理论全集允许 2-key 候选 —— 见
//!                      CandidateEnumerationSpec::MONOTONE_V2_THEORETICAL)
//! target universe    = 全部词目标 减去 已入库 ZERO_REGRESSION production 词
//!                      (优化前排除,不是 assignment 后过滤)
//! production filter  = shortcut 长度 >= PRODUCTION_MIN_SHORTCUT_LENGTH(3)
//!                      且 baseline_fanout > 0(均为优化前 policy 过滤;
//!                      1/2 键空间保留给一级简码与单字)
//! candidate universe = baseline_fanout > 0
//!                      (优化前移除空码候选,不污染 greedy allocation)
//! fanout 上限        = 无(selection-cost 模型对任意 depth 都有定义)
//! reference run      = OperatingPointId::Balanced × normalized(50:50,Conservative)
//! robustness         = 30 次 normalized 增量运行的同码票数,
//!                      整数交叉乘法 votes × 5 >= total_runs × 4
//! net utility        = reference assignment eligible(净收益 > 0)
//! 兼容门             = pre-FIXED_FIRST current production fanout == baseline fanout
//! ```
//!
//! candidate grammar(结构合法性)与 production filter(长度/重码策略)
//! 严格分层:语法层 `时间 → uij/FI` 与 `uj/II` 都是理论候选,`ujm/IF`
//! 结构性非法;production 最短长度过滤在优化前移除 `uj`,不改变语法
//! 语义。grammar 与枚举规格由 [`collect_fixed_first_evidence`] 硬断言。
//!
//! selection-cost 模型对任意 depth 都有定义(rank 1 / 2..=9 / >=10 三档),
//! 因此 candidate universe 不设 fanout 上限;深度分布只在 audit 中如实报告。
//!
//! 选择基于 [`CodeOccupancy::build_baseline_fixed`] + frozen ZERO_REGRESSION
//! 词/码集合,绝不基于含本层结果的 current-production state(否则导出会
//! 自引用消失)。policy 语义变化视为 policy version change + canonical data
//! review,不静默重新生成用户肌肉记忆数据。

use std::collections::{BTreeMap, BTreeSet};

use xhup_core::KeySequence;
use xhup_generator::canonical_word_shortcut_entries;

use crate::AnalysisData;
use crate::candidates::{CandidateEnumerationSpec, CandidateGrammar, WordTarget};
use crate::frequency::{CharCodeUsage, FrequencyScale};
use crate::occupancy::{CodeOccupancy, CollisionClass};
use crate::optimize::{OptimizationProfile, ShortcutAssignment};
use crate::production::{
    ROBUSTNESS_DENOMINATOR, ROBUSTNESS_NUMERATOR, SENSITIVITY_RUNS, reference_scale,
};
use crate::sweep::{OperatingPointId, WordRobustness, robustness_map, run_normalized_grid};

/// Production FIXED_FIRST selection policy 的稳定版本标识(写入 canonical TSV 头)。
pub const FIXED_FIRST_PRODUCTION_POLICY_VERSION: &str = "fixed-first-high-v1";

/// PR #23 production 最短 shortcut 长度(1/2 键空间保留给一级简码与单字)。
///
/// 这是 production policy,不属于候选语法:Monotone V2 语法理论全集允许
/// 2-key 候选(如 `时间 → uj/II`),它们可被分析/审计,但不出现在
/// production candidate universe 中。未来如研究 2-key 词语简码,只改本
/// policy,不改语法。
pub const PRODUCTION_MIN_SHORTCUT_LENGTH: usize = 3;

/// incremental universe 统计(解释 universe shrinkage 的 audit)。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FixedFirstUniverseStats {
    /// 全部词目标数(100k production 词)。
    pub original_targets: usize,
    /// 因已有 ZERO_REGRESSION production 简码而被移除的目标数(优化前)。
    pub zr_words_excluded: usize,
    /// 剩余目标数。
    pub remaining_targets: usize,
    /// 因短于 production 最短长度被过滤的候选数(如 2-key `uj/II`;
    /// 语法理论候选,production policy 排除)。
    pub below_min_length_candidates: usize,
    /// 进入优化的重码候选数(baseline fanout > 0;无上限)。
    pub colliding_candidates: usize,
    /// 过滤后不再有任何候选的目标数。
    pub targets_without_candidates: usize,
    /// 进入优化的候选按码长分布(下标 3..=7)。
    pub candidate_lengths: [usize; 8],
}

/// 构造 incremental FIXED_FIRST target/candidate universe。
///
/// 三个限制都发生在 optimizer 之前:先移除已有 ZERO_REGRESSION 简码的词,
/// 再对剩余词移除短于 production 最短长度的候选(2-key 语法理论候选,
/// 如 `时间 → uj/II`),最后只保留 `baseline fanout > 0` 的重码候选。
/// `data` 的候选枚举规格必须是 Monotone V2 理论全集(production 最短
/// 长度是本函数的 policy 过滤,不由语法表达)。
pub fn build_fixed_first_universe(
    data: &AnalysisData,
) -> (Vec<WordTarget>, FixedFirstUniverseStats) {
    let zr_words: BTreeSet<&str> = canonical_word_shortcut_entries()
        .iter()
        .map(|entry| entry.word())
        .collect();

    let mut stats = FixedFirstUniverseStats {
        original_targets: data.targets.len(),
        ..FixedFirstUniverseStats::default()
    };
    let mut targets = data.targets.clone();
    let before = targets.len();
    targets.retain(|target| !zr_words.contains(target.word()));
    stats.zr_words_excluded = before - targets.len();
    stats.remaining_targets = targets.len();

    for target in &mut targets {
        target.retain_candidates(|candidate| {
            let length_ok = candidate.shortcut_code().len() >= PRODUCTION_MIN_SHORTCUT_LENGTH;
            if !length_ok {
                stats.below_min_length_candidates += 1;
            }
            length_ok && data.occupancy.fanout(candidate.shortcut_code()) > 0
        });
        stats.colliding_candidates += target.candidates().len();
        for candidate in target.candidates() {
            stats.candidate_lengths[candidate.shortcut_code().len()] += 1;
        }
    }
    stats.targets_without_candidates = targets
        .iter()
        .filter(|target| target.candidates().is_empty())
        .count();
    (targets, stats)
}

/// production FIXED_FIRST selection 的完整证据。
pub struct FixedFirstEvidence {
    /// reference 运行的 typed operating-point identity(恒为 Balanced)。
    pub reference_point: OperatingPointId,
    /// 候选语法身份(恒为 MonotoneSuffixInitialsV2;production 证据显式
    /// 暴露所用语法,不允许隐式)。
    pub candidate_grammar: CandidateGrammar,
    /// reference run 的 assignments(typed 选取)。
    pub reference_assignments: Vec<ShortcutAssignment>,
    /// 30 次 normalized 增量运行的逐词稳健性。
    pub robustness: BTreeMap<String, WordRobustness>,
    /// incremental universe 统计。
    pub universe: FixedFirstUniverseStats,
}

/// 收集 production FIXED_FIRST 选择证据:30 次 normalized 增量主网格
/// (grid 中的 Balanced/50:50/Conservative 运行同时即 reference run)。
///
/// `data` 的候选枚举规格必须是 Monotone V2 理论全集(grammar 绑定硬断言;
/// production 最短长度过滤发生在 [`build_fixed_first_universe`])。
pub fn collect_fixed_first_evidence(data: &AnalysisData) -> FixedFirstEvidence {
    assert_eq!(
        data.enumeration_spec,
        CandidateEnumerationSpec::MONOTONE_V2_THEORETICAL,
        "FIXED_FIRST production evidence 必须绑定 monotone-suffix-initials-v2 \
         理论全集枚举规格"
    );
    let (targets, universe) = build_fixed_first_universe(data);
    let runs = run_normalized_grid(
        &targets,
        &data.occupancy,
        &data.frequency,
        &[OptimizationProfile::FixedFirst],
    );
    assert_eq!(
        runs.len(),
        SENSITIVITY_RUNS,
        "FIXED_FIRST normalized 增量主网格应恰为 {SENSITIVITY_RUNS} 次运行"
    );
    let reference = runs
        .iter()
        .find(|run| {
            run.point == OperatingPointId::Balanced
                && matches!(
                    run.scale,
                    FrequencyScale::Normalized { char_share, usage }
                        if char_share == 0.5 && usage == CharCodeUsage::Conservative
                )
        })
        .expect("不变量:主网格必然包含 Balanced/50:50/Conservative reference 运行");
    let robustness = robustness_map(&runs, OptimizationProfile::FixedFirst);
    FixedFirstEvidence {
        reference_point: reference.point,
        candidate_grammar: CandidateGrammar::MonotoneSuffixInitialsV2,
        reference_assignments: reference.outcome.assignments.clone(),
        robustness,
        universe,
    }
}

/// 一条 production FIXED_FIRST 简码关系(canonical TSV 的一行 + audit 字段)。
#[derive(Clone, Debug)]
pub struct FixedFirstSelection {
    /// 词语。
    pub word: String,
    /// 完整码(保留;shortcut 是新增别名,不是替换)。
    pub full_code: KeySequence,
    /// shortcut 码(= reference run assignment 码)。
    pub shortcut_code: KeySequence,
    /// F/I 投影模式(如 `FI`)。
    pub mode: String,
    /// 万象聚合频率分数(仅 audit,不入 TSV)。
    pub frequency_score: u64,
    /// shortcut 码在 baseline fixed occupancy 中的 fanout(>= 1,无上限)。
    pub baseline_fanout: usize,
    /// 期望名次 = baseline_fanout + 1。
    pub expected_rank: usize,
    /// baseline 碰撞类型(原来撞了谁;仅 audit)。
    pub baseline_collision_class: CollisionClass,
    /// reference 净收益(> 0;仅 audit)。
    pub net_utility: f64,
    /// robustness 最多票码的选中票数。
    pub top_code_votes: usize,
    /// sensitivity 总运行数。
    pub total_runs: usize,
}

/// production FIXED_FIRST gate 的排除原因(selection audit 分类统计)。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum FixedFirstExclusionReason {
    /// 词已有 ZERO_REGRESSION production 简码(优化前过滤;理论 0)。
    AlreadyZeroRegressionWord,
    /// 候选不与 baseline fixed 冲突(优化前过滤;理论 0)。
    NoCollidingCandidate,
    /// reference assignment 存在,但增量运行中无该词记录。
    NoRobustnessEvidence,
    /// robustness 最多票码与 reference assignment 码不一致。
    TopCodeMismatch,
    /// 同码票数低于 4/5(整数交叉乘法判定)。
    BelowThreshold,
    /// reference 净收益非正(eligible 已保证 > 0;理论 0)。
    NonPositiveReferenceUtility,
    /// pre-FIXED_FIRST current production fanout != baseline fanout(理论 0)。
    CurrentOccupancyMismatch,
    /// shortcut 码与 ZERO_REGRESSION production 码冲突(理论 0)。
    ZeroRegressionCodeConflict,
}

/// selection audit 计数。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FixedFirstSelectionAudit {
    /// reference run 的 assignment 数。
    pub reference_assignments: usize,
    /// 增量运行中有记录的不同词数。
    pub robustness_records: usize,
    /// 进入 production 的条数。
    pub selected: usize,
    /// 各排除原因的条数。
    pub excluded: BTreeMap<FixedFirstExclusionReason, usize>,
}

impl FixedFirstSelectionAudit {
    /// 被排除的总条数。
    pub fn excluded_total(&self) -> usize {
        self.excluded.values().sum()
    }

    /// 某个原因的排除条数。
    pub fn excluded_by(&self, reason: FixedFirstExclusionReason) -> usize {
        self.excluded.get(&reason).copied().unwrap_or(0)
    }
}

/// 一条被 production gate 排除的 reference assignment(audit 用)。
#[derive(Clone, Debug)]
pub struct FixedFirstExclusion {
    /// 词语。
    pub word: String,
    /// reference run 分配的 shortcut 码。
    pub reference_code: KeySequence,
    /// 万象聚合频率分数。
    pub frequency_score: u64,
    /// baseline fanout。
    pub baseline_fanout: usize,
    /// robustness 最多票码(无记录时为 `None`)。
    pub top_code: Option<String>,
    /// 最多票码的选中票数与总运行数(无记录时为 (0, 0))。
    pub votes: (usize, usize),
    /// 排除原因。
    pub reason: FixedFirstExclusionReason,
}

/// production FIXED_FIRST 选择的完整结果。
pub struct FixedFirstProductionSelection {
    /// 进入 production 的简码集(canonical 序列化顺序)。
    pub selected: Vec<FixedFirstSelection>,
    /// 被排除的 reference assignments(含原因,audit 用)。
    pub exclusions: Vec<FixedFirstExclusion>,
    /// 计数审计。
    pub audit: FixedFirstSelectionAudit,
}

/// 按冻结 policy 从证据中选择 production FIXED_FIRST 简码集。
///
/// `baseline` 必须是 baseline fixed occupancy;`pre_fixed_first_current`
/// 必须是 PR #22 后的 current production occupancy(baseline + ZR 层),
/// 仅用于兼容性断言(每个 FF 码:current fanout == baseline fanout),
/// 不参与选择本身。
pub fn select_fixed_first_production(
    evidence: &FixedFirstEvidence,
    baseline: &CodeOccupancy,
    pre_fixed_first_current: &CodeOccupancy,
) -> FixedFirstProductionSelection {
    use FixedFirstExclusionReason as Reason;

    let zr_words: BTreeSet<&str> = canonical_word_shortcut_entries()
        .iter()
        .map(|entry| entry.word())
        .collect();
    let zr_codes: BTreeSet<KeySequence> = canonical_word_shortcut_entries()
        .iter()
        .map(|entry| entry.shortcut_code().clone())
        .collect();

    let mut selected = Vec::new();
    let mut exclusions = Vec::new();
    let mut audit = FixedFirstSelectionAudit {
        reference_assignments: evidence.reference_assignments.len(),
        robustness_records: evidence.robustness.len(),
        ..FixedFirstSelectionAudit::default()
    };
    for assignment in &evidence.reference_assignments {
        let shortcut_code = &assignment.evaluation.shortcut_code;
        let baseline_fanout = baseline.fanout(shortcut_code);
        let record = evidence.robustness.get(&assignment.word);
        let top = record.and_then(|r| r.top_code());
        let votes = top.map(|(_, v)| v).unwrap_or(0);
        let total_runs = record.map_or(0, |r| r.total_runs);
        let reason = if zr_words.contains(assignment.word.as_str()) {
            Some(Reason::AlreadyZeroRegressionWord)
        } else if baseline_fanout == 0 {
            Some(Reason::NoCollidingCandidate)
        } else if pre_fixed_first_current.fanout(shortcut_code) != baseline_fanout {
            Some(Reason::CurrentOccupancyMismatch)
        } else if zr_codes.contains(shortcut_code) {
            Some(Reason::ZeroRegressionCodeConflict)
        } else if record.is_none() || top.is_none() {
            Some(Reason::NoRobustnessEvidence)
        } else if top.is_some_and(|(code, _)| code != shortcut_code.to_string()) {
            Some(Reason::TopCodeMismatch)
        } else if votes * ROBUSTNESS_DENOMINATOR < total_runs * ROBUSTNESS_NUMERATOR {
            // 整数交叉乘法:votes/total >= 4/5 ⇔ votes×5 >= total×4。
            Some(Reason::BelowThreshold)
        } else if assignment.evaluation.breakdown.net_utility <= 0.0 {
            Some(Reason::NonPositiveReferenceUtility)
        } else {
            None
        };
        if let Some(reason) = reason {
            *audit.excluded.entry(reason).or_default() += 1;
            exclusions.push(FixedFirstExclusion {
                word: assignment.word.clone(),
                reference_code: shortcut_code.clone(),
                frequency_score: assignment.frequency_score,
                baseline_fanout,
                top_code: top.map(|(code, _)| code.to_string()),
                votes: (votes, total_runs),
                reason,
            });
            continue;
        }
        selected.push(FixedFirstSelection {
            word: assignment.word.clone(),
            full_code: assignment.full_code.clone(),
            shortcut_code: shortcut_code.clone(),
            mode: assignment.evaluation.mode.clone(),
            frequency_score: assignment.frequency_score,
            baseline_fanout,
            expected_rank: baseline_fanout + 1,
            baseline_collision_class: baseline.collision_class(shortcut_code),
            net_utility: assignment.evaluation.breakdown.net_utility,
            top_code_votes: votes,
            total_runs,
        });
    }
    audit.selected = selected.len();
    // 三重唯一不变量(reference run 已保证词/码唯一,production set 再硬断言)。
    let mut words = BTreeSet::new();
    let mut codes = BTreeSet::new();
    let mut word_full_codes = BTreeSet::new();
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
        assert!(
            word_full_codes.insert((&entry.word, &entry.full_code)),
            "production (词, 完整码) 重复: {}",
            entry.word
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
    FixedFirstProductionSelection {
        selected,
        exclusions,
        audit,
    }
}

/// FIXED_FIRST 收益审计(reference 尺度,无量纲)。
#[derive(Clone, Copy, Debug, Default)]
pub struct FixedFirstBenefitAudit {
    /// 全部词目标(100k universe)的频率加权按键(baseline,完整码)。
    pub weighted_keys_before: f64,
    /// production FIXED_FIRST 集合的 raw 加权按键节省(纯键数差)。
    pub production_raw_keys_saved: f64,
    /// production FIXED_FIRST 集合的有效模型收益
    /// (含 selection/ambiguity 成本;existing fixed 次序不变,disruption = 0)。
    pub production_effective_benefit: f64,
}

/// 计算 FIXED_FIRST 收益审计。combined 口径见 CLI(与 ZR 层共用同一
/// 100k universe denominator,不做百分比相加)。
pub fn fixed_first_benefit_audit(
    data: &AnalysisData,
    selection: &FixedFirstProductionSelection,
) -> FixedFirstBenefitAudit {
    let scale = reference_scale();
    let weight = |score: u64| data.frequency.target_weight(&scale, score);
    let before: f64 = data
        .targets
        .iter()
        .map(|t| weight(t.frequency_score()) * t.full_code().len() as f64)
        .sum();
    let raw_saved: f64 = selection
        .selected
        .iter()
        .map(|e| weight(e.frequency_score) * (e.full_code.len() - e.shortcut_code.len()) as f64)
        .sum();
    // net_utility 已是频率加权净收益(disruption = 0,故等于有效模型收益)。
    let effective: f64 = selection.selected.iter().map(|e| e.net_utility).sum();
    FixedFirstBenefitAudit {
        weighted_keys_before: before,
        production_raw_keys_saved: raw_saved,
        production_effective_benefit: effective,
    }
}

/// 序列化为 canonical TSV:`词<TAB>完整码<TAB>shortcut 码<TAB>模式`。
///
/// UTF-8、LF、无 BOM、恰好一个末尾换行;`#` 头记录 policy 与 provenance,
/// 不含时间戳、主机、路径等易变内容。fanout/rank/utility/votes 等 analysis
/// evidence 不属于 production semantic identity,一律不写入。
pub fn serialize_fixed_first_tsv(selected: &[FixedFirstSelection]) -> String {
    let mut out = String::new();
    out.push_str("# XHUP Flow high-robustness FIXED_FIRST word shortcuts.\n");
    out.push_str("# Source universe: data/words/wanxiang_base_words.tsv\n");
    out.push_str("# Existing production words excluded: data/shortcuts/word_zero_regression.tsv\n");
    out.push_str(
        "# Selection: FIXED_FIRST / colliding-only (baseline fanout > 0) / balanced / normalized 50:50 conservative\n",
    );
    out.push_str("# candidate grammar: ");
    out.push_str(CandidateGrammar::MonotoneSuffixInitialsV2.label());
    out.push('\n');
    out.push_str("# production min shortcut length: ");
    out.push_str(&PRODUCTION_MIN_SHORTCUT_LENGTH.to_string());
    out.push('\n');
    out.push_str(
        "# Robustness gate: same-code stability >= 4/5 over 30 normalized sensitivity runs\n",
    );
    out.push_str("# policy: ");
    out.push_str(FIXED_FIRST_PRODUCTION_POLICY_VERSION);
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

/// runtime 全量 A/B 审计 manifest:`shortcut 码<TAB>词<TAB>baseline fanout
/// <TAB>期望名次<TAB>baseline 碰撞类型<TAB>baseline 菜单(逗号分隔,名次序)`。
///
/// 这是分析证据,供 `tests/librime` 的 CONTROL/PRODUCTION runtime 对照使用;
/// 不写入仓库,由调用方输出到临时路径。baseline 菜单文本取自 baseline
/// occupancy 的组内名次序(显式权重降序)。
pub fn fixed_first_audit_manifest(
    selected: &[FixedFirstSelection],
    baseline: &CodeOccupancy,
) -> String {
    let mut out = String::new();
    for entry in selected {
        let group = baseline
            .group(&entry.shortcut_code)
            .expect("不变量:FIXED_FIRST 码在 baseline 必然有候选组");
        let menu: Vec<&str> = group.iter().map(|c| c.text()).collect();
        debug_assert_eq!(menu.len(), entry.baseline_fanout);
        out.push_str(&entry.shortcut_code.to_string());
        out.push('\t');
        out.push_str(&entry.word);
        out.push('\t');
        out.push_str(&entry.baseline_fanout.to_string());
        out.push('\t');
        out.push_str(&entry.expected_rank.to_string());
        out.push('\t');
        out.push_str(entry.baseline_collision_class.label());
        out.push('\t');
        out.push_str(&menu.join(","));
        out.push('\n');
    }
    out
}
