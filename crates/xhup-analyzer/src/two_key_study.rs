//! 二码词语简码研究:MonotoneSuffixInitialsV2 语法下 2 字词 `II` 理论
//! 候选的可行性分析,以及空码 zero-regression 生产层的数据基础。
//!
//! study `two-key-word-shortcut-v1`(研究身份;若生产化,生产 policy 为
//! `two-key-zero-regression-v1`,见 `production_two_key`)。
//!
//! 本模块是纯分析:不修改任何 production 产物;全部证据从 generator
//! 只读投影与 canonical 数据现算。
//!
//! 架构分层:
//!
//! - **候选全集**:2 字词 × Monotone V2 语法 × `II` 模式 × 2 键码。
//!   每个规范 2 字词恰有一个 II 理论候选(语法保证,硬断言);3/4 字词
//!   不属于本研究对象(无长度 2 的单调候选)。
//! - **当前最优静态路径**:每词从 full code / ZR 简码 / FIXED_FIRST
//!   简码三条现有静态路径中,按真实有效成本(rank/fanout/selection/
//!   ambiguity)取最小。不假设更短必然更优。
//! - **2 键单字 domain**:独立的 2 码单字归一化频率域(Σ P = 1)。
//!   现有 3 码 domain 语义(`frequency.rs`)不动。
//! - **SAFE_APPEND**(研究):既有 2 键单字保持精确次序,词追加到组尾
//!   (rank = fanout + 1),扰动恒为 0。
//! - **OPTIMAL_INSERT**(研究,非 production 政策):对每码枚举插入名次
//!   1..=fanout+1,量化「若允许重排 2 键单字」的理论上限,含真实
//!   被置换单字的频率加权扰动成本。
//!
//! 研究宇宙:Universe A(无既有 ZR/FF 简码的词,production 兼容)与
//! Universe B(全部 2 字词,研究上限),指标分别报告、分母不混用。

use std::collections::BTreeMap;

use xhup_core::KeySequence;
use xhup_generator::{
    canonical_fixed_first_shortcut_entries, canonical_word_shortcut_entries,
    char_code_analysis_entries, word_code_analysis_entries,
};

use crate::candidates::{CandidateEnumerationSpec, CandidateGrammar};
use crate::cost::{CostBreakdown, CostModel};
use crate::frequency::{CharCodeUsage, FrequencyModel, FrequencyScale};
use crate::occupancy::CodeOccupancy;
use crate::sweep::OperatingPointId;

/// 研究身份标识(报告头用;生产 policy 身份见 `production_two_key`)。
pub const TWO_KEY_STUDY_VERSION: &str = "two-key-word-shortcut-v1";

/// 敏感性网格规模:5 operating points × 3 混合比 × 2 归属假设。
pub const SENSITIVITY_RUNS: usize = 30;

// ── 2 键单字 domain ─────────────────────────────────────────

/// 2 码单字关系的归一化概率域(独立于 3 码 domain,语义不动后者)。
///
/// 每条 2 码关系是 `(汉字, 完整双拼音码)`。与 3 码层不同,2 码层上
/// 每个读音恰好映射到一个音码,不存在「同读音多个形码」的归属歧义,
/// 因此 Conservative 与 Split 在本域内重合(两者都等于逐条目塌缩分数
/// 的和;`finalize` 已保证同读音多形路径不重复计分)。这一事实在报告
/// 中如实记录,usage 轴保留是为了与 3 码网格形状对称。
#[derive(Clone, Debug)]
pub struct TwoKeyCharDomain {
    /// 全部 2 码单字关系(分析证据投影)。
    entries: Vec<CharCodeAnalysisEntryOwned>,
    /// 归一化分母(全部 2 码关系频率分数之和)。
    total: u64,
}

/// 内部拥有的 2 码单字关系(避免生命周期穿透整个研究结构)。
#[derive(Clone, Debug)]
struct CharCodeAnalysisEntryOwned {
    hanzi: char,
    code: KeySequence,
    frequency_score: u64,
    rime_weight: u32,
}

impl TwoKeyCharDomain {
    /// 从 generator 只读投影构建 2 码单字 domain。
    pub fn build() -> Self {
        let entries: Vec<CharCodeAnalysisEntryOwned> = char_code_analysis_entries()
            .iter()
            .filter(|entry| entry.code().len() == 2)
            .map(|entry| CharCodeAnalysisEntryOwned {
                hanzi: entry.hanzi().as_char(),
                code: entry.code().clone(),
                frequency_score: entry.frequency_score(),
                rime_weight: entry.rime_weight(),
            })
            .collect();
        let total: u64 = entries.iter().map(|e| e.frequency_score).sum();
        TwoKeyCharDomain { entries, total }
    }

    /// 2 码单字关系数。
    pub fn relation_count(&self) -> usize {
        self.entries.len()
    }

    /// 全部 2 码单字关系。
    pub fn entries(&self) -> impl Iterator<Item = (char, &KeySequence, u64, u32)> + '_ {
        self.entries
            .iter()
            .map(|e| (e.hanzi, &e.code, e.frequency_score, e.rime_weight))
    }

    /// 指定混合比下的单条 2 码关系权重:char_share × P_2key(关系)。
    ///
    /// Conservative 与 Split 在本域重合(见类型文档),usage 仅作网格
    /// 对称参数,不影响数值。
    pub fn relation_weight(
        &self,
        char_share: f64,
        hanzi: char,
        code: &KeySequence,
        frequency_score: u64,
        _usage: CharCodeUsage,
    ) -> f64 {
        char_share * self.relation_probability(hanzi, code, frequency_score)
    }

    /// 2 码关系的 domain 内归一化概率。
    fn relation_probability(&self, _hanzi: char, _code: &KeySequence, frequency_score: u64) -> f64 {
        frequency_score as f64 / self.total.max(1) as f64
    }
}

// ── 当前最优静态路径 ─────────────────────────────────────────

/// 一条现有静态输入路径。
#[derive(Clone, Debug)]
pub struct StaticRoute {
    /// 路径类型。
    pub kind: RouteKind,
    /// 路径码(完整码或简码)。
    pub code: KeySequence,
    /// 组内名次(真实 production 状态)。
    pub rank: u32,
    /// 组内候选数。
    pub fanout: usize,
    /// 有效成本(typed + selection + ambiguity)。
    pub cost: CostBreakdown,
}

/// 静态路径类型。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum RouteKind {
    /// 4/6/8 键完整词码(固定词层)。
    FullCode,
    /// ZERO_REGRESSION 简码(PR #22)。
    ZeroRegression,
    /// FIXED_FIRST 简码(PR #23)。
    FixedFirst,
}

impl RouteKind {
    /// 报告用稳定标签。
    pub fn label(self) -> &'static str {
        match self {
            RouteKind::FullCode => "FULL_CODE",
            RouteKind::ZeroRegression => "ZR_SHORTCUT",
            RouteKind::FixedFirst => "FIXED_FIRST_SHORTCUT",
        }
    }
}

/// 词的既有简码状态。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExistingShortcutStatus {
    /// 无任何 production 词语简码。
    None,
    /// 已有 ZERO_REGRESSION 简码。
    ZeroRegression {
        /// 简码。
        code: KeySequence,
    },
    /// 已有 FIXED_FIRST 简码。
    FixedFirst {
        /// 简码。
        code: KeySequence,
    },
}

impl ExistingShortcutStatus {
    /// 是否无既有简码(Universe A 成员资格)。
    pub fn is_none(&self) -> bool {
        matches!(self, ExistingShortcutStatus::None)
    }
}

// ── 研究候选 ─────────────────────────────────────────────────

/// 一条 2 键 II 研究候选(研究语义,非 production 推荐)。
#[derive(Clone, Debug)]
pub struct TwoKeyCandidate {
    /// 词语(恰 2 字)。
    pub word: String,
    /// 完整码(4 键,canonical 固定词层)。
    pub full_code: KeySequence,
    /// II 理论简码(2 键)。
    pub two_key_code: KeySequence,
    /// 万象聚合频率分数。
    pub frequency_score: u64,
    /// 既有简码状态。
    pub existing_shortcut: ExistingShortcutStatus,
    /// 当前最优静态路径。
    pub current_best: StaticRoute,
    /// 2 键单字占用类别。
    pub code_class: TwoKeyCodeClass,
    /// 该码既有 2 键单字数(空码为 0)。
    pub char_fanout: usize,
}

/// 2 键码占用类别。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TwoKeyCodeClass {
    /// 2 键 exact-code 空间完全空闲(无任何既有 exact 关系)。
    Empty,
    /// 已有 2 键单字占用。
    OccupiedByChars,
}

impl TwoKeyCodeClass {
    /// 报告用稳定标签。
    pub fn label(self) -> &'static str {
        match self {
            TwoKeyCodeClass::Empty => "EMPTY_2KEY_CODE",
            TwoKeyCodeClass::OccupiedByChars => "OCCUPIED_2KEY_CODE",
        }
    }
}

/// 研究全集(两个宇宙共用同一候选集,按宇宙过滤统计)。
pub struct TwoKeyUniverse {
    /// 全部 II 候选(word 字典序)。
    pub candidates: Vec<TwoKeyCandidate>,
    /// 2 键单字 domain。
    pub char_domain: TwoKeyCharDomain,
    /// 词 domain 频率模型(重用,用于 word 侧权重)。
    pub frequency: FrequencyModel,
    /// 当前真实生产占用(路径/占用证据)。
    pub occupancy: CodeOccupancy,
    /// 全部 2 字词目标数(与候选数相等,断言)。
    pub two_char_target_count: usize,
}

impl TwoKeyUniverse {
    /// 构建研究全集:枚举 Monotone V2 II 候选 + 真实占用/路径证据。
    ///
    /// 硬断言(违反即研究前提损坏,STOP):
    /// - 每个规范 2 字词恰有一个 II 理论候选(模式 II、长度 2、
    ///   机械首键投影一致);
    /// - 3/4 字词不产生任何 2 键候选;
    /// - 全部候选模式属于 MonotoneSuffixInitialsV2。
    pub fn build() -> Self {
        let words = word_code_analysis_entries();
        let chars = char_code_analysis_entries();
        let frequency = FrequencyModel::build(&chars, &words);
        let occupancy = CodeOccupancy::build_current_production();
        let char_domain = TwoKeyCharDomain::build();

        // 既有 ZR/FF 简码索引(词 → 状态)。
        let zr: BTreeMap<String, KeySequence> = canonical_word_shortcut_entries()
            .iter()
            .map(|entry| (entry.word().to_string(), entry.shortcut_code().clone()))
            .collect();
        let ff: BTreeMap<String, KeySequence> = canonical_fixed_first_shortcut_entries()
            .iter()
            .map(|entry| (entry.word().to_string(), entry.shortcut_code().clone()))
            .collect();

        // Monotone V2 枚举(全部词;后续只保留 2 字 II)。
        let (targets, _) = crate::candidates::enumerate_targets_with_spec(
            &words,
            CandidateEnumerationSpec::MONOTONE_V2_THEORETICAL,
        );

        let mut candidates = Vec::new();
        let mut two_char_target_count = 0usize;
        for target in &targets {
            let char_count = target.word().chars().count();
            if char_count != 2 {
                // 3/4 字词不得产生 2 键候选。
                for candidate in target.candidates() {
                    assert!(
                        candidate.shortcut_code().len() != 2,
                        "非 2 字词不得有 2 键候选: {}",
                        target.word()
                    );
                }
                continue;
            }
            two_char_target_count += 1;
            // 恰一个 II 候选。
            let ii: Vec<_> = target
                .candidates()
                .iter()
                .filter(|c| c.mode().pattern() == "II" && c.shortcut_code().len() == 2)
                .collect();
            assert_eq!(
                ii.len(),
                1,
                "2 字词必须恰有一个 II 理论候选: {}",
                target.word()
            );
            let candidate = ii[0];
            let two_key_code = candidate.shortcut_code().clone();
            // 机械首键投影:full[0] + full[2]。
            let full = target.full_code().as_slice();
            assert_eq!(
                two_key_code.as_slice(),
                &[full[0], full[2]],
                "II 投影必须等于两字首键: {}",
                target.word()
            );
            // 语法合法(防 Legacy 混入)。
            assert!(
                CandidateGrammar::MonotoneSuffixInitialsV2.accepts(candidate.mode()),
                "II 模式必须属于 MonotoneSuffixInitialsV2"
            );

            let existing_shortcut = match (zr.get(target.word()), ff.get(target.word())) {
                (Some(code), _) => ExistingShortcutStatus::ZeroRegression { code: code.clone() },
                (None, Some(code)) => ExistingShortcutStatus::FixedFirst { code: code.clone() },
                (None, None) => ExistingShortcutStatus::None,
            };

            let current_best = current_best_route(
                target.word(),
                target.full_code(),
                &existing_shortcut,
                &occupancy,
            );

            let char_fanout = occupancy.fanout(&two_key_code);
            let code_class = if char_fanout == 0 {
                TwoKeyCodeClass::Empty
            } else {
                TwoKeyCodeClass::OccupiedByChars
            };

            candidates.push(TwoKeyCandidate {
                word: target.word().to_string(),
                full_code: target.full_code().clone(),
                two_key_code,
                frequency_score: target.frequency_score(),
                existing_shortcut,
                current_best,
                code_class,
                char_fanout,
            });
        }
        assert_eq!(
            candidates.len(),
            two_char_target_count,
            "全部 2 字词都应有 II 候选"
        );
        candidates.sort_by(|a, b| a.word.cmp(&b.word));

        TwoKeyUniverse {
            candidates,
            char_domain,
            frequency,
            occupancy,
            two_char_target_count,
        }
    }

    /// Universe A 成员(无既有 ZR/FF 简码)。
    pub fn universe_a(&self) -> impl Iterator<Item = &TwoKeyCandidate> {
        self.candidates
            .iter()
            .filter(|c| c.existing_shortcut.is_none())
    }

    /// Universe B 成员(全部 2 字词)。
    pub fn universe_b(&self) -> impl Iterator<Item = &TwoKeyCandidate> {
        self.candidates.iter()
    }
}

/// 计算词的当前最优静态路径(full / ZR / FF 三者真实有效成本取最小)。
///
/// 平局确定:按 FULL → ZR → FF 的路径序,再按码字典序。
fn current_best_route(
    word: &str,
    full_code: &KeySequence,
    existing: &ExistingShortcutStatus,
    occupancy: &CodeOccupancy,
) -> StaticRoute {
    // balanced 成本模型(static 研究的统一尺度;网格另做敏感性)。
    let cost = OperatingPointId::Balanced.operating_point().cost_model();
    let mut best: Option<StaticRoute> = None;
    let mut consider = |kind: RouteKind, code: &KeySequence| {
        let group = occupancy
            .group(code)
            .unwrap_or_else(|| panic!("不变量:路径码必然有占用组: {word} {code}"));
        let rank = group
            .iter()
            .find(|c| c.text() == word)
            .unwrap_or_else(|| panic!("不变量:词必然占用其路径码组: {word} {code}"))
            .rank();
        let route = StaticRoute {
            kind,
            code: code.clone(),
            rank,
            fanout: group.len(),
            cost: cost.baseline_cost(code.len(), rank, group.len()),
        };
        let better = match &best {
            None => true,
            Some(current) => {
                route.cost.total() < current.cost.total()
                    || (route.cost.total() == current.cost.total() && route.kind < current.kind)
            }
        };
        if better {
            best = Some(route);
        }
    };
    consider(RouteKind::FullCode, full_code);
    if let ExistingShortcutStatus::ZeroRegression { code } = existing {
        consider(RouteKind::ZeroRegression, code);
    }
    if let ExistingShortcutStatus::FixedFirst { code } = existing {
        consider(RouteKind::FixedFirst, code);
    }
    best.expect("至少有完整码路径")
}

// ── 研究场景 ─────────────────────────────────────────────────

/// 2 键词候选的放置场景。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TwoKeyPlacement {
    /// 安全追加:既有单字次序不变,词 rank = fanout + 1(生产安全上界)。
    SafeAppend,
    /// 最优插入(研究,非 production 政策):允许重排,取净收益最大名次。
    OptimalInsert,
}

/// 一个场景下的候选评估结果。
#[derive(Clone, Debug)]
pub struct PlacementEvaluation {
    /// 放置场景。
    pub placement: TwoKeyPlacement,
    /// 目标名次(SAFE_APPEND 恒为 fanout+1;OPTIMAL_INSERT 为最优名次)。
    pub rank: u32,
    /// 插入后 projected fanout(= char_fanout + 1)。
    pub projected_fanout: usize,
    /// 2 键路径有效成本。
    pub cost: CostBreakdown,
    /// 相对当前最优静态路径的频率加权净收益(word 域权重)。
    pub weighted_net_gain: f64,
    /// (仅 OPTIMAL_INSERT)被置换单字的频率加权扰动成本(char 域)。
    pub weighted_char_disruption: f64,
    /// (仅 OPTIMAL_INSERT)被置换单字数(名次 >= 插入名次者)。
    pub displaced_char_count: usize,
    /// (仅 OPTIMAL_INSERT)被置换的单字(名次序,审计用)。
    pub displaced_chars: Vec<char>,
}

/// 在指定尺度/成本模型下评估一个候选的放置场景。
///
/// SAFE_APPEND:既有单字扰动恒为 0(构造保证:仅追加组尾,不重排)。
/// OPTIMAL_INSERT:枚举 rank 1..=fanout+1,对每个 rank 计算
/// `词收益 − 被置换单字扰动`,取最大(平局取最小名次,确定性)。
#[allow(clippy::too_many_arguments)]
pub fn evaluate_placement(
    candidate: &TwoKeyCandidate,
    placement: TwoKeyPlacement,
    occupancy: &CodeOccupancy,
    char_domain: &TwoKeyCharDomain,
    frequency: &FrequencyModel,
    scale: &FrequencyScale,
    cost: &CostModel,
) -> PlacementEvaluation {
    let FrequencyScale::Normalized { char_share, usage } = *scale else {
        panic!("研究仅支持 Normalized 尺度");
    };
    let word_weight = frequency.target_weight(scale, candidate.frequency_score);
    let current_cost = candidate.current_best.cost.total();

    let group = occupancy.group(&candidate.two_key_code);
    let char_fanout = group.map_or(0, |g| g.len());
    let char_weight = |text: &str, code: &KeySequence, score: u64| -> f64 {
        let hanzi = text.chars().next().expect("单字候选恰为一字");
        char_domain.relation_weight(char_share, hanzi, code, score, usage)
    };

    match placement {
        TwoKeyPlacement::SafeAppend => {
            let rank = u32::try_from(char_fanout + 1).expect("名次超出 u32");
            let route_cost = cost.shortcut_cost(2, rank, char_fanout + 1, &empty_ii_mode());
            PlacementEvaluation {
                placement,
                rank,
                projected_fanout: char_fanout + 1,
                cost: route_cost,
                weighted_net_gain: word_weight * (current_cost - route_cost.total()),
                weighted_char_disruption: 0.0,
                displaced_char_count: 0,
                displaced_chars: Vec::new(),
            }
        }
        TwoKeyPlacement::OptimalInsert => {
            let group = group.unwrap_or(&[]);
            let mut best: Option<PlacementEvaluation> = None;
            for insert_rank in 1..=(char_fanout + 1) {
                let rank = u32::try_from(insert_rank).expect("名次超出 u32");
                let route_cost = cost.shortcut_cost(2, rank, char_fanout + 1, &empty_ii_mode());
                let word_gain = word_weight * (current_cost - route_cost.total());
                // 被置换单字:原名次 >= insert_rank 者全部 +1;
                // 全组 ambiguity 从 fanout → fanout+1。
                let mut disruption = 0.0f64;
                let mut displaced = Vec::new();
                for existing in group {
                    let old_rank = existing.rank();
                    if (old_rank as usize) < insert_rank {
                        continue;
                    }
                    let new_rank = old_rank + 1;
                    let delta = (cost.selection_cost(new_rank)
                        + cost.ambiguity_cost(char_fanout + 1))
                        - (cost.selection_cost(old_rank) + cost.ambiguity_cost(char_fanout));
                    if delta <= 0.0 {
                        continue;
                    }
                    disruption += char_weight(
                        existing.text(),
                        &candidate.two_key_code,
                        existing.frequency_score(),
                    ) * delta
                        * cost.disruption_coeff;
                    displaced.push(existing.text().chars().next().expect("单字候选恰为一字"));
                }
                let net = word_gain - disruption;
                let evaluation = PlacementEvaluation {
                    placement,
                    rank,
                    projected_fanout: char_fanout + 1,
                    cost: route_cost,
                    weighted_net_gain: net,
                    weighted_char_disruption: disruption,
                    displaced_char_count: displaced.len(),
                    displaced_chars: displaced,
                };
                let better = match &best {
                    None => true,
                    Some(current) => {
                        net > current.weighted_net_gain
                            || (net == current.weighted_net_gain && rank < current.rank)
                    }
                };
                if better {
                    best = Some(evaluation);
                }
            }
            best.expect("插入名次空间非空")
        }
    }
}

/// II 模式的零切换 ShortcutMode(mode_complexity 恒 0;成本场景用)。
///
/// 构造真实的 2 字 II 模式(两字都取 Initial),保持类型诚实;
/// transitions() == 0,任何 mode_complexity_per_transition 下成本恒 0。
fn empty_ii_mode() -> crate::candidates::ShortcutMode {
    use crate::candidates::Mode;
    crate::candidates::ShortcutMode::from_modes_for_study(&[Mode::Initial, Mode::Initial])
}

// ── 敏感性网格 ───────────────────────────────────────────────

/// 一次研究网格运行(一个尺度/成本组合)。
pub struct TwoKeyStudyRun {
    /// operating point typed identity。
    pub point: OperatingPointId,
    /// 频率尺度。
    pub scale: FrequencyScale,
    /// 每个候选(索引对齐 universe.candidates)的 SAFE_APPEND 评估。
    pub safe: Vec<PlacementEvaluation>,
    /// 每个候选的 OPTIMAL_INSERT 评估。
    pub optimal: Vec<PlacementEvaluation>,
}

/// 敏感性网格:5 × 3 × 2 = 30 次运行(与 PR22/23 网格形状一致)。
pub fn run_two_key_grid(universe: &TwoKeyUniverse) -> Vec<TwoKeyStudyRun> {
    let mut runs = Vec::new();
    for point in crate::sweep::operating_points() {
        let cost = point.cost_model();
        for (_, char_share) in crate::sweep::mixtures() {
            for usage in [CharCodeUsage::Conservative, CharCodeUsage::Split] {
                let scale = FrequencyScale::Normalized { char_share, usage };
                let safe = universe
                    .candidates
                    .iter()
                    .map(|c| {
                        evaluate_placement(
                            c,
                            TwoKeyPlacement::SafeAppend,
                            &universe.occupancy,
                            &universe.char_domain,
                            &universe.frequency,
                            &scale,
                            &cost,
                        )
                    })
                    .collect();
                let optimal = universe
                    .candidates
                    .iter()
                    .map(|c| {
                        evaluate_placement(
                            c,
                            TwoKeyPlacement::OptimalInsert,
                            &universe.occupancy,
                            &universe.char_domain,
                            &universe.frequency,
                            &scale,
                            &cost,
                        )
                    })
                    .collect();
                runs.push(TwoKeyStudyRun {
                    point: point.id,
                    scale,
                    safe,
                    optimal,
                });
            }
        }
    }
    assert_eq!(
        runs.len(),
        SENSITIVITY_RUNS,
        "研究网格应恰为 {SENSITIVITY_RUNS} 次运行"
    );
    runs
}

/// 每候选的网格稳健性摘要(参考运行 = Balanced × 50:50 × Conservative)。
pub struct TwoKeyRobustness {
    /// 参考运行索引(网格内)。
    pub reference_index: usize,
    /// SAFE_APPEND:参考净收益是否为正。
    pub safe_positive: bool,
    /// SAFE_APPEND:网格中净收益为正的运行数。
    pub safe_positive_runs: usize,
    /// OPTIMAL_INSERT:参考净收益是否为正。
    pub optimal_positive: bool,
    /// OPTIMAL_INSERT:网格中净收益为正的运行数。
    pub optimal_positive_runs: usize,
    /// OPTIMAL_INSERT:最优名次在网格中与参考一致的运行数。
    pub optimal_rank_stable_runs: usize,
}

/// 计算参考运行索引(Balanced × 0.50 × Conservative)。
pub fn reference_run_index(runs: &[TwoKeyStudyRun]) -> usize {
    runs.iter()
        .position(|run| {
            run.point == OperatingPointId::Balanced
                && matches!(
                    run.scale,
                    FrequencyScale::Normalized { char_share, usage }
                        if char_share == 0.5 && usage == CharCodeUsage::Conservative
                )
        })
        .expect("不变量:网格必然包含 Balanced/50:50/Conservative 参考运行")
}

/// 计算每个候选的稳健性摘要(索引对齐 universe.candidates)。
pub fn robustness_summary(runs: &[TwoKeyStudyRun]) -> Vec<TwoKeyRobustness> {
    let reference = reference_run_index(runs);
    (0..runs[reference].safe.len())
        .map(|index| {
            let safe_positive = runs[reference].safe[index].weighted_net_gain > 0.0;
            let safe_positive_runs = runs
                .iter()
                .filter(|run| run.safe[index].weighted_net_gain > 0.0)
                .count();
            let optimal_positive = runs[reference].optimal[index].weighted_net_gain > 0.0;
            let optimal_positive_runs = runs
                .iter()
                .filter(|run| run.optimal[index].weighted_net_gain > 0.0)
                .count();
            let reference_rank = runs[reference].optimal[index].rank;
            let optimal_rank_stable_runs = runs
                .iter()
                .filter(|run| run.optimal[index].rank == reference_rank)
                .count();
            TwoKeyRobustness {
                reference_index: reference,
                safe_positive,
                safe_positive_runs,
                optimal_positive,
                optimal_positive_runs,
                optimal_rank_stable_runs,
            }
        })
        .collect()
}
