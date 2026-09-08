//! v2 映射产出器(评估用,非 production 冻结):候选位资源贪心分配。
//!
//! 设计文档:docs/optimizer-v2.md §1/§2。与 v1(optimize.rs)的关键区别:
//! 码位是**有序候选位资源** —— 允许把词分配到已被占用码的 rank 2/3,
//! 竞争与扰动成本由 [`CostModelV2::selection_cost`] /
//! [`CostModelV2::disruption_cost`] 显式建模,效用由
//! [`evaluate_assignment`] 计算。
//!
//! 算法(确定性贪心):
//!
//! 1. **评分**(静态):每个 (词, 候选码) 在 baseline 固定层占用上评估全部
//!    可行候选位 rank 1..=MAX_RANK,取效用最高的 rank。候选码集合沿用
//!    candidates.rs 的枚举管线(本模块不发明新编码规则)。
//! 2. **接纳规则**:效用必须严格优于「不分配」的对照 —— 该词留在全码
//!    (rank 1 空位槽)的效用;净效用 ≤ 0 的分配丢弃。频率项在同词对照中
//!    抵消,因此 EvidenceWeights 通过**跨词竞争排序**生效(步骤 3 的顺序
//!    键含绝对效用)。
//! 3. **贪心分配**:按 (绝对效用降序, 净效用降序, 词升序, 码升序) 全序
//!    遍历;每词最多分配一次;同码多词按质量(ln1p 归一化频率)决定最终
//!    候选次序(baseline 候选与 v2 词混排,同分 baseline 在前、v2 词按
//!    词形升序),最终 rank 由全部分配完成后的码内次序重排回填。
//!
//! 确定性硬要求:同输入同参数 → 字节一致输出。全部遍历走切片/BTreeMap,
//! 浮点比较用 `total_cmp`,排序键以 (词, 码) 全序兜底。

use std::collections::{BTreeMap, BTreeSet};

use xhup_core::KeySequence;

use crate::candidates::WordTarget;
use crate::evidence::LexicalEvidence;
use crate::occupancy::{CandidateSource, CodeOccupancy};
use crate::optimizer_v2::{CostModelV2, EvidenceWeights, UtilityBreakdownV2, evaluate_assignment};
use crate::xhup_prior::{NEUTRAL_PRIOR, XhupStylePrior};

/// 建模的候选位上限(与 `CostModelV2::rank_cost` 覆盖范围一致)。
pub const MAX_RANK: usize = 4;

/// 固定层码位占用质量视图:码 → 既有候选的效用质量列表(组内名次升序)。
///
/// 质量 = ln1p(domain 内归一化频率),与 [`evaluate_assignment`] 的频率项
/// 同尺度;词语与单字各自 domain 归一化(跨 domain 绝对尺度不可比,见
/// frequency.rs 文档)。构建一次,全部扫描运行点复用(与参数无关)。
pub struct BaselineMassView {
    groups: BTreeMap<KeySequence, Vec<f64>>,
}

impl BaselineMassView {
    /// 从固定层占用构建(domain 归一化分母在占用全量上现算)。
    pub fn build(occupancy: &CodeOccupancy) -> Self {
        let mut word_total = 0u64;
        let mut char_total = 0u64;
        for code in occupancy.occupied_codes() {
            for candidate in occupancy.group(code).unwrap_or(&[]) {
                match candidate.source() {
                    CandidateSource::CharCode => char_total += candidate.frequency_score(),
                    CandidateSource::FixedWord
                    | CandidateSource::WordShortcut
                    | CandidateSource::FixedFirstWordShortcut => {
                        word_total += candidate.frequency_score()
                    }
                    CandidateSource::Level1Shortcut => {}
                }
            }
        }
        let mass = |source: CandidateSource, score: u64| {
            let total = match source {
                CandidateSource::CharCode => char_total,
                CandidateSource::FixedWord
                | CandidateSource::WordShortcut
                | CandidateSource::FixedFirstWordShortcut => word_total,
                CandidateSource::Level1Shortcut => 0,
            };
            if total == 0 {
                0.0
            } else {
                (score as f64 / total as f64).ln_1p()
            }
        };
        let mut groups = BTreeMap::new();
        for code in occupancy.occupied_codes() {
            let masses: Vec<f64> = occupancy
                .group(code)
                .unwrap_or(&[])
                .iter()
                .map(|c| mass(c.source(), c.frequency_score()))
                .collect();
            groups.insert(code.clone(), masses);
        }
        BaselineMassView { groups }
    }

    /// 测试构造:显式给定 码 → 质量列表(组内名次升序)。
    #[doc(hidden)]
    pub fn for_test(groups: Vec<(KeySequence, Vec<f64>)>) -> Self {
        BaselineMassView {
            groups: groups.into_iter().collect(),
        }
    }

    /// 某码的既有候选质量(组内名次升序);空闲码返回空切片。
    pub fn group(&self, code: &KeySequence) -> &[f64] {
        self.groups.get(code).map_or(&[], Vec::as_slice)
    }
}

/// 一条 v2 分配:词 → (码, 最终候选位)。
#[derive(Clone, Debug)]
pub struct MappingV2Entry {
    /// 词语。
    pub word: String,
    /// 分配的 shortcut 码。
    pub code: KeySequence,
    /// 最终候选位(全部分配完成后码内质量次序重排;1 = 首选)。
    pub rank: usize,
    /// 净效用(相对「留在全码」对照;恒正)。
    pub net_utility: f64,
    /// 评估时的效用分解(可解释性;评估基于评估时点占用,最终 rank 可能
    /// 因后续同码分配而后移)。
    pub breakdown: UtilityBreakdownV2,
}

/// v2 映射:词 → 分配(BTreeMap 序,序列化字节确定)。
pub struct MappingV2 {
    entries: BTreeMap<String, MappingV2Entry>,
}

impl MappingV2 {
    /// 分配数。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 查词。
    pub fn get(&self, word: &str) -> Option<&MappingV2Entry> {
        self.entries.get(word)
    }

    /// 全部条目(词升序)。
    pub fn entries(&self) -> impl Iterator<Item = &MappingV2Entry> {
        self.entries.values()
    }

    /// 序列化为确定性 TSV:`词<TAB>码<TAB>最终rank<TAB>净效用`。
    pub fn to_tsv(&self) -> String {
        let mut out = String::from("word\tcode\trank\tnet_utility\n");
        for entry in self.entries.values() {
            out.push_str(&format!(
                "{}\t{}\t{}\t{:.6}\n",
                entry.word, entry.code, entry.rank, entry.net_utility
            ));
        }
        out
    }

    /// 回放视图输入:(词, 键数, 候选位) 列表(词升序,确定性)。
    pub fn replay_plans(&self) -> Vec<(String, usize, usize)> {
        self.entries
            .values()
            .map(|e| (e.word.clone(), e.code.len(), e.rank))
            .collect()
    }

    /// 候选 fanout 统计:v2 词在码位上的分布(已用码数/共享码数/均值/最大)。
    pub fn fanout_stats(&self) -> FanoutStats {
        let mut by_code: BTreeMap<&KeySequence, usize> = BTreeMap::new();
        for entry in self.entries.values() {
            *by_code.entry(&entry.code).or_default() += 1;
        }
        let codes_used = by_code.len();
        let shared_codes = by_code.values().filter(|&&n| n >= 2).count();
        let max = by_code.values().copied().max().unwrap_or(0);
        let mean = if codes_used == 0 {
            0.0
        } else {
            self.entries.len() as f64 / codes_used as f64
        };
        FanoutStats {
            codes_used,
            shared_codes,
            mean,
            max,
        }
    }
}

/// 候选 fanout 统计。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FanoutStats {
    /// 被 v2 词占用的码数。
    pub codes_used: usize,
    /// 承载 ≥2 个 v2 词的码数(v2 候选位资源特征的直接体现)。
    pub shared_codes: usize,
    /// 平均每码 v2 词数。
    pub mean: f64,
    /// 单码最大 v2 词数。
    pub max: usize,
}

/// 评分阶段的一条可行分配(净效用 > 0)。
struct ScoredAssignment {
    word: String,
    code: KeySequence,
    /// 绝对效用(跨词竞争排序键;含频率项)。
    utility: f64,
    /// 净效用(相对全码对照;接纳判定)。
    net_utility: f64,
    /// 码内质量排序键(与 baseline 质量同尺度)。
    mass: f64,
    breakdown: UtilityBreakdownV2,
}

/// 确定性贪心分配:词 → (码, 预期 rank)。
///
/// - `targets`:候选词集合(candidates.rs 管线产出);
/// - `evidence`:词 → 多信号词汇证据(缺失词不参与);
/// - `baseline`:固定层码位占用质量视图;
/// - `prior`:XHUP 风格先验(无参考数据传 `None`,全部中立 0.5)。
pub fn produce_mapping(
    targets: &[WordTarget],
    evidence: &BTreeMap<String, LexicalEvidence>,
    baseline: &BaselineMassView,
    cost: &CostModelV2,
    weights: &EvidenceWeights,
    prior: Option<&XhupStylePrior>,
) -> MappingV2 {
    let prior_score = |word: &str, code: &KeySequence, rank: usize| {
        prior.map_or(NEUTRAL_PRIOR, |p| p.score(word, &code.to_string(), rank))
    };

    // 阶段 1:静态评分(对每个候选码评估全部可行候选位,取最优)。
    let mut scored: Vec<ScoredAssignment> = Vec::new();
    for target in targets {
        let Some(ev) = evidence.get(target.word()) else {
            continue;
        };
        let mass = ev.normalized_frequency().ln_1p();
        let full_len = target.full_code().len();
        // 「不分配」对照:留在全码,rank 1 空位槽。
        let baseline_total = evaluate_assignment(
            ev,
            &crate::optimizer_v2::CandidateSlot {
                key_len: full_len,
                rank: 1,
                occupant_mass: 0.0,
            },
            cost,
            weights,
            prior_score(target.word(), target.full_code(), 1),
            full_len,
            true,
        )
        .total();

        for candidate in target.candidates() {
            let code = candidate.shortcut_code();
            let group = baseline.group(code);
            let pattern_consistent = target.full_code().as_slice().starts_with(code.as_slice());
            let mut best: Option<(usize, UtilityBreakdownV2)> = None;
            for rank in 1..=(group.len() + 1).min(MAX_RANK) {
                // 扰动质量:插入 rank 后被挤后的既有候选质量合计。
                let occupant_mass: f64 = group[rank - 1..].iter().sum();
                let breakdown = evaluate_assignment(
                    ev,
                    &crate::optimizer_v2::CandidateSlot {
                        key_len: code.len(),
                        rank,
                        occupant_mass,
                    },
                    cost,
                    weights,
                    prior_score(target.word(), code, rank),
                    full_len,
                    pattern_consistent,
                );
                let better = match &best {
                    None => true,
                    Some((best_rank, best_breakdown)) => breakdown
                        .total()
                        .total_cmp(&best_breakdown.total())
                        .then(best_rank.cmp(&rank))
                        .is_gt(),
                };
                if better {
                    best = Some((rank, breakdown));
                }
            }
            let Some((_rank, breakdown)) = best else {
                continue;
            };
            let net_utility = breakdown.total() - baseline_total;
            if net_utility > 0.0 {
                scored.push(ScoredAssignment {
                    word: target.word().to_string(),
                    code: code.clone(),
                    utility: breakdown.total(),
                    net_utility,
                    mass,
                    breakdown,
                });
            }
        }
    }

    // 阶段 2:确定性全序贪心(绝对效用降序 → 净效用降序 → 词 → 码)。
    scored.sort_by(|a, b| {
        b.utility
            .total_cmp(&a.utility)
            .then(b.net_utility.total_cmp(&a.net_utility))
            .then(a.word.cmp(&b.word))
            .then(a.code.cmp(&b.code))
    });

    let mut assigned: BTreeSet<String> = BTreeSet::new();
    // 码 → 已分配 v2 词 (质量, 词)(插入序 = 效用序;最终次序末尾重排)。
    let mut code_members: BTreeMap<KeySequence, Vec<(f64, String)>> = BTreeMap::new();
    let mut chosen: BTreeMap<String, ScoredAssignment> = BTreeMap::new();
    for candidate in scored {
        if assigned.contains(candidate.word.as_str()) {
            continue;
        }
        let members = code_members.entry(candidate.code.clone()).or_default();
        // 码内插入位:既有候选(质量严格更高,或同分 baseline/词形序在前)之后。
        let position = merged_position(baseline.group(&candidate.code), members, &candidate);
        if position > MAX_RANK {
            continue;
        }
        assigned.insert(candidate.word.clone());
        members.push((candidate.mass, candidate.word.clone()));
        chosen.insert(candidate.word.clone(), candidate);
    }

    // 最终 rank 回填:全部分配完成后按码内质量次序重排。
    let mut entries = BTreeMap::new();
    for (word, candidate) in chosen {
        let members = &code_members[&candidate.code];
        let rank = merged_position(baseline.group(&candidate.code), members, &candidate);
        entries.insert(
            word.clone(),
            MappingV2Entry {
                word,
                code: candidate.code,
                rank,
                net_utility: candidate.net_utility,
                breakdown: candidate.breakdown,
            },
        );
    }
    MappingV2 { entries }
}

/// 候选在码内合并次序中的 1 起始位次:baseline 质量 + v2 成员按质量降序
/// 混排(同分 baseline 在前;v2 词同分按词形升序)。
fn merged_position(
    baseline: &[f64],
    members: &[(f64, String)],
    candidate: &ScoredAssignment,
) -> usize {
    let before_baseline = baseline
        .iter()
        .filter(|m| m.total_cmp(&candidate.mass).is_gt())
        .count();
    let before_v2 = members
        .iter()
        .filter(|(m, w)| {
            m.total_cmp(&candidate.mass).is_gt()
                || (m.total_cmp(&candidate.mass).is_eq() && *w < candidate.word)
        })
        .count();
    before_baseline + before_v2 + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candidates::ShortcutCandidate;

    fn evidence_map(entries: Vec<LexicalEvidence>) -> BTreeMap<String, LexicalEvidence> {
        entries
            .into_iter()
            .map(|e| (e.word().to_string(), e))
            .collect()
    }

    fn word_evidence(word: &str, code: &str, normalized: f64) -> LexicalEvidence {
        LexicalEvidence::for_test(
            word,
            code.parse().unwrap(),
            1000,
            normalized,
            None,
            None,
            None,
        )
    }

    fn target(word: &str, full: &str, candidates: &[&str]) -> WordTarget {
        WordTarget::with_candidates_for_test(
            word,
            full.parse().unwrap(),
            1000,
            candidates
                .iter()
                .map(|c| ShortcutCandidate::for_test(c.parse().unwrap()))
                .collect(),
        )
    }

    fn empty_baseline() -> BaselineMassView {
        BaselineMassView::for_test(Vec::new())
    }

    #[test]
    fn determinism_byte_identical() {
        let targets = vec![
            target("我们", "womf", &["wm", "wom"]),
            target("时间", "uijm", &["uj", "uij"]),
        ];
        let evidence = evidence_map(vec![
            word_evidence("我们", "womf", 1e-4),
            word_evidence("时间", "uijm", 8e-5),
        ]);
        let run = || {
            produce_mapping(
                &targets,
                &evidence,
                &empty_baseline(),
                &CostModelV2::default(),
                &EvidenceWeights::default(),
                None,
            )
            .to_tsv()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn occupied_code_accepts_second_word_at_rank2() {
        // 两词竞争同一空码:质量高者 rank 1,另一词占用 rank 2(v2 与 v1
        // 码位唯一的核心区别)。
        let targets = vec![
            target("我们", "womf", &["wm"]),
            target("时间", "uijm", &["wm"]),
        ];
        let evidence = evidence_map(vec![
            word_evidence("我们", "womf", 1e-4),
            word_evidence("时间", "uijm", 8e-5),
        ]);
        let mapping = produce_mapping(
            &targets,
            &evidence,
            &empty_baseline(),
            &CostModelV2::default(),
            &EvidenceWeights::default(),
            None,
        );
        let women = mapping.get("我们").expect("我们 应被分配");
        let shijian = mapping.get("时间").expect("时间 应被分配");
        assert_eq!(women.rank, 1);
        assert_eq!(shijian.rank, 2, "同码第二词应占用 rank 2");
        let fanout = mapping.fanout_stats();
        assert_eq!(fanout.codes_used, 1);
        assert_eq!(fanout.shared_codes, 1);
    }

    #[test]
    fn non_positive_net_utility_is_dropped() {
        // 认知惩罚拉满(候选码非全码前缀)→ 净效用 ≤ 0,分配丢弃。
        let targets = vec![target("我们", "womf", &["wm"])];
        let evidence = evidence_map(vec![word_evidence("我们", "womf", 1e-4)]);
        let cost = CostModelV2 {
            cognitive_complexity_coeff: 100.0,
            ..CostModelV2::default()
        };
        let mapping = produce_mapping(
            &targets,
            &evidence,
            &empty_baseline(),
            &cost,
            &EvidenceWeights::default(),
            None,
        );
        assert!(mapping.is_empty(), "净效用 ≤ 0 的分配必须丢弃");
    }

    #[test]
    fn baseline_occupants_push_rank_down() {
        // baseline 占用质量高于新词 → 新词在其后。
        let code: KeySequence = "wm".parse().unwrap();
        let baseline = BaselineMassView::for_test(vec![(code, vec![(1e-3f64).ln_1p()])]);
        let targets = vec![target("我们", "womf", &["wm"])];
        let evidence = evidence_map(vec![word_evidence("我们", "womf", 1e-4)]);
        let mapping = produce_mapping(
            &targets,
            &evidence,
            &baseline,
            &CostModelV2::default(),
            &EvidenceWeights::default(),
            None,
        );
        let entry = mapping.get("我们").expect("我们 应被分配");
        assert_eq!(entry.rank, 2, "baseline 占用者应占据 rank 1");
    }
}
