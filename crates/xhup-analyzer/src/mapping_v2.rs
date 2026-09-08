//! v2 映射产出器(评估用,非 production 冻结):候选位资源贪心分配。
//!
//! 设计文档:docs/optimizer-v2.md §1/§2。与 v1(optimize.rs)的关键区别:
//! 码位是**有序候选位资源** —— 允许把词分配到已被占用码的 rank 2/3,
//! 竞争与扰动成本由 [`CostModelV2::selection_cost`] /
//! [`CostModelV2::disruption_cost`] 显式建模,效用由
//! [`evaluate_assignment`] 计算。
//!
//! ## 质量尺度(2026-10 修复,扫描退化根因)
//!
//! 归一化频率是 1e-5 量级的概率,`ln1p(p) ≈ p`;若直接用作占用质量,
//! `ambiguity_coeff × occupant_mass` 与 `disruption_coeff × occupant_mass`
//! 比 rank/键成本(O(1))低五个数量级,两个系数结构性失效(扫描网格
//! 退化)。因此本模块的质量统一锚定到 **domain 中位频率**:
//! `mass = ln1p(p / p_median)` —— 中位词 0.69、头部词 3~6、长尾趋 0,
//! 与键/候选位成本同量级,竞争与扰动定价才真正进入决策。
//!
//! ## 算法(确定性贪心)
//!
//! 1. **评分**(静态):每个 (词, 候选码) 在 baseline 固定层占用上评估,
//!    产出 (绝对效用, 词, 码) 全序短名单。候选码沿用 candidates.rs 枚举
//!    管线(本模块不发明新编码规则)。
//! 2. **状态依赖接纳**(动态):按短名单顺序遍历;每条的占用质量取**当时**
//!    状态(baseline + 已分配 v2 词)的码内总质量(保守上界:扰动成本按
//!    码内全部占用计费,不只被挤后的部分);候选位 = 码内质量混排的实际
//!    位次。**候选位不回退守卫**:分配位次不得差于该词全码的真实组内
//!    rank(否则回放按最小期望成本选路时,rank≥2 的便宜简码会挤掉全码
//!    rank1 方案,首选命中崩塌);净效用 = 该槽位效用 − 该词「留在全码
//!    真实 rank」的对照效用;净效用 ≤ 0 或位次越界则跳过(词的其他
//!    候选码仍有机会)。
//! 3. **最终 rank 回填**:全部分配完成后按码内质量次序重排回填。
//!
//! ## 参数影响路径(哪些参数如何生效)
//!
//! - `rank_cost` / `ambiguity_coeff` / `disruption_coeff`:候选位成本与
//!   扰动定价,直接决定已占用码的接纳(质量尺度修复后生效);
//! - `xhup_deviation_coeff`:仅在有参考映射时生效(无参考时先验全中立,
//!   同词对照两侧精确抵消 —— 扫描时该维度退化为重复点,属预期行为);
//! - `EvidenceWeights`:频率项在同词对照中抵消,因此经两条路径生效 —
//!   跨词竞争排序键(绝对效用)+ 词的多信号质量(码内位次与占用质量);
//! - 频率在同词接纳判定中结构性抵消:是否给词分配简码由槽位资源定价
//!   决定,频率决定的是**谁抢到稀缺位**。
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

/// 多信号质量尺度:各频率类信号的 domain 中位锚点(见模块文档「质量
/// 尺度」)。构建一次,全部运行点复用。
pub struct MassScale {
    /// 全局归一化频率中位。
    word_ref: f64,
    /// 会话域频率中位(仅已测量条目;无测量时 1.0,ln1p(p/1)≈0)。
    conversation_ref: f64,
    /// 句子覆盖度中位。
    coverage_ref: f64,
    /// 上下文多样性中位。
    diversity_ref: f64,
}

/// 中位锚点(最近秩;空集 → 1.0,使 ln1p(p/ref) 退化为 ≈0 而非除零)。
fn median_anchor(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return 1.0;
    }
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

impl MassScale {
    /// 从证据视图现算各信号中位锚点。
    pub fn build(evidence: &BTreeMap<String, LexicalEvidence>) -> Self {
        MassScale {
            word_ref: median_anchor(
                evidence
                    .values()
                    .map(|e| e.normalized_frequency())
                    .collect(),
            ),
            conversation_ref: median_anchor(
                evidence
                    .values()
                    .filter_map(|e| e.conversation_frequency())
                    .collect(),
            ),
            coverage_ref: median_anchor(
                evidence
                    .values()
                    .filter_map(|e| e.sentence_coverage())
                    .collect(),
            ),
            diversity_ref: median_anchor(
                evidence
                    .values()
                    .filter_map(|e| e.context_diversity().map(|d| d as f64))
                    .collect(),
            ),
        }
    }

    /// 测试构造:显式给定各锚点。
    #[doc(hidden)]
    pub fn for_test(
        word_ref: f64,
        conversation_ref: f64,
        coverage_ref: f64,
        diversity_ref: f64,
    ) -> Self {
        MassScale {
            word_ref,
            conversation_ref,
            coverage_ref,
            diversity_ref,
        }
    }

    /// 词的多信号质量:有效权重加权的 ln1p(信号/中位锚点) 之和。
    ///
    /// EvidenceWeights 经此进入码内位次竞争与占用质量(缺失信号按
    /// [`EvidenceWeights::effective`] 重归一化)。
    pub fn mass(&self, evidence: &LexicalEvidence, weights: &EvidenceWeights) -> f64 {
        let (wg, wc, wsc, wcd) = weights.effective(evidence);
        let scaled =
            |signal: Option<f64>, anchor: f64| signal.map_or(0.0, |v| (v / anchor).ln_1p());
        wg * (evidence.normalized_frequency() / self.word_ref).ln_1p()
            + wc * scaled(evidence.conversation_frequency(), self.conversation_ref)
            + wsc * scaled(evidence.sentence_coverage(), self.coverage_ref)
            + wcd
                * scaled(
                    evidence.context_diversity().map(|d| d as f64),
                    self.diversity_ref,
                )
    }
}

/// 固定层码位占用质量视图(构建一次,各运行点复用,与参数无关)。
///
/// 质量与 [`MassScale`] 同尺度(ln1p(p / domain 中位));baseline 候选只有
/// 全局频率一个信号(语料信号不覆盖单字与全部词),故只用全局域锚点。
pub struct BaselineMassView {
    /// 码 → 既有候选质量(组内名次升序)。
    groups: BTreeMap<KeySequence, Vec<f64>>,
    /// 码 → 既有候选质量合计(竞争强度,占用质量上界)。
    total_mass: BTreeMap<KeySequence, f64>,
    /// 词 → 其全码组内真实 rank(接纳对照槽位用)。
    full_code_ranks: BTreeMap<String, usize>,
}

impl BaselineMassView {
    /// 从固定层占用构建(domain 中位锚点在占用全量上现算)。
    pub fn build(occupancy: &CodeOccupancy) -> Self {
        let mut word_scores: Vec<f64> = Vec::new();
        let mut char_scores: Vec<f64> = Vec::new();
        let mut word_total = 0u64;
        let mut char_total = 0u64;
        for code in occupancy.occupied_codes() {
            for candidate in occupancy.group(code).unwrap_or(&[]) {
                match candidate.source() {
                    CandidateSource::CharCode => {
                        char_total += candidate.frequency_score();
                        char_scores.push(candidate.frequency_score() as f64);
                    }
                    CandidateSource::FixedWord
                    | CandidateSource::WordShortcut
                    | CandidateSource::FixedFirstWordShortcut => {
                        word_total += candidate.frequency_score();
                        word_scores.push(candidate.frequency_score() as f64);
                    }
                    CandidateSource::Level1Shortcut => {}
                }
            }
        }
        let word_ref = median_anchor(word_scores) / word_total.max(1) as f64;
        let char_ref = median_anchor(char_scores) / char_total.max(1) as f64;
        let mass = |source: CandidateSource, score: u64| {
            let (total, anchor) = match source {
                CandidateSource::CharCode => (char_total, char_ref),
                CandidateSource::FixedWord
                | CandidateSource::WordShortcut
                | CandidateSource::FixedFirstWordShortcut => (word_total, word_ref),
                CandidateSource::Level1Shortcut => (0, 1.0),
            };
            if total == 0 {
                0.0
            } else {
                (score as f64 / total as f64 / anchor).ln_1p()
            }
        };
        let mut groups = BTreeMap::new();
        let mut total_mass = BTreeMap::new();
        let mut full_code_ranks = BTreeMap::new();
        for code in occupancy.occupied_codes() {
            let group = occupancy.group(code).unwrap_or(&[]);
            let masses: Vec<f64> = group
                .iter()
                .map(|c| mass(c.source(), c.frequency_score()))
                .collect();
            total_mass.insert(code.clone(), masses.iter().sum());
            // 词的全码 rank 只数词域内候选:replay 全码层按词词典视图(不
            // 含同码单字),守卫口径必须与回放选路口径一致,否则同码单字
            // 会把词的全码 rank 抬高一位,放行「简码 demote 首选」。
            let mut word_rank = 0usize;
            for candidate in group {
                if candidate.source() == CandidateSource::FixedWord {
                    word_rank += 1;
                    full_code_ranks.insert(candidate.text().to_string(), word_rank);
                }
            }
            groups.insert(code.clone(), masses);
        }
        BaselineMassView {
            groups,
            total_mass,
            full_code_ranks,
        }
    }

    /// 测试构造:显式给定 码 → 质量列表(组内名次升序)与 词 → 全码 rank。
    #[doc(hidden)]
    pub fn for_test(
        groups: Vec<(KeySequence, Vec<f64>)>,
        full_code_ranks: Vec<(&str, usize)>,
    ) -> Self {
        let total_mass = groups
            .iter()
            .map(|(code, masses)| (code.clone(), masses.iter().sum()))
            .collect();
        BaselineMassView {
            groups: groups.into_iter().collect(),
            total_mass,
            full_code_ranks: full_code_ranks
                .into_iter()
                .map(|(w, r)| (w.to_string(), r))
                .collect(),
        }
    }

    /// 某码的既有候选质量(组内名次升序);空闲码返回空切片。
    pub fn group(&self, code: &KeySequence) -> &[f64] {
        self.groups.get(code).map_or(&[], Vec::as_slice)
    }

    /// 某码的既有占用质量合计(竞争强度)。
    pub fn total_mass(&self, code: &KeySequence) -> f64 {
        self.total_mass.get(code).copied().unwrap_or(0.0)
    }

    /// 词在其全码组内的真实 rank(未知词 = 1,保守)。
    pub fn full_code_rank(&self, word: &str) -> usize {
        self.full_code_ranks.get(word).copied().unwrap_or(1)
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
    /// 净效用(相对「留在全码真实 rank」对照;恒正)。
    pub net_utility: f64,
    /// 分配时点状态的效用分解(可解释性;最终 rank 可能因后续同码
    /// 分配而后移)。
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

    /// 明细 TSV(人工审查用):在 [`Self::to_tsv`] 基础上带效用分解各
    /// 项与主导项(docs/optimizer-v2.md §6 理由卡)。
    pub fn to_detail_tsv(&self) -> String {
        let mut out = String::from(
            "word\tcode\trank\tnet_utility\tfrequency_utility\tkeystrokes_saved\txhup_prior\tselection_cost\tdisruption_cost\tkeystroke_cost\trare_pollution\tdominant\n",
        );
        for entry in self.entries.values() {
            let b = &entry.breakdown;
            out.push_str(&format!(
                "{}\t{}\t{}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{}\n",
                entry.word,
                entry.code,
                entry.rank,
                entry.net_utility,
                b.frequency_utility,
                b.keystrokes_saved,
                b.xhup_prior,
                b.selection_cost,
                b.disruption_cost,
                b.keystroke_cost,
                b.rare_pollution,
                dominant_term(b),
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

/// 效用分解的主导项(绝对值最大者,同值取列表先者;dump 可解释性用,
/// docs §6 理由卡)。
pub fn dominant_term(breakdown: &UtilityBreakdownV2) -> &'static str {
    let terms = [
        ("frequency_utility", breakdown.frequency_utility),
        ("keystrokes_saved", breakdown.keystrokes_saved),
        ("xhup_prior", breakdown.xhup_prior),
        ("selection_cost", -breakdown.selection_cost),
        ("disruption_cost", -breakdown.disruption_cost),
        ("keystroke_cost", -breakdown.keystroke_cost),
        ("rare_pollution", -breakdown.rare_pollution),
    ];
    let mut best = terms[0];
    for term in terms {
        if term.1.abs().total_cmp(&best.1.abs()).is_gt() {
            best = term;
        }
    }
    best.0
}

/// 评分短名单的一条记录(绝对效用为排序键;接纳在分配时点重判)。
struct ScoredAssignment {
    word: String,
    code: KeySequence,
    full_code: KeySequence,
    /// 绝对效用(跨词竞争排序键;含频率项)。
    utility: f64,
    /// 词的多信号质量(码内位次与占用质量)。
    mass: f64,
    /// 「留在全码真实 rank」的对照效用(每词一次,随记录携带)。
    baseline_total: f64,
}

/// 确定性贪心分配:词 → (码, 预期 rank)。
///
/// - `targets`:候选词集合(candidates.rs 管线产出);
/// - `evidence`:词 → 多信号词汇证据(缺失词不参与);
/// - `scale`:多信号质量尺度(中位锚点);
/// - `baseline`:固定层码位占用质量视图;
/// - `prior`:XHUP 风格先验(无参考数据传 `None`,全部中立 0.5)。
pub fn produce_mapping(
    targets: &[WordTarget],
    evidence: &BTreeMap<String, LexicalEvidence>,
    scale: &MassScale,
    baseline: &BaselineMassView,
    cost: &CostModelV2,
    weights: &EvidenceWeights,
    prior: Option<&XhupStylePrior>,
) -> MappingV2 {
    let prior_score = |word: &str, code: &KeySequence, rank: usize| {
        prior.map_or(NEUTRAL_PRIOR, |p| p.score(word, &code.to_string(), rank))
    };

    // 阶段 1:静态评分短名单(占用质量取 baseline 上界,排序用)。
    let mut scored: Vec<ScoredAssignment> = Vec::new();
    for target in targets {
        let Some(ev) = evidence.get(target.word()) else {
            continue;
        };
        let mass = scale.mass(ev, weights);
        let full_len = target.full_code().len();
        // 「不分配」对照:留在全码,取真实组内 rank(非理想化 rank 1)。
        let baseline_total = evaluate_assignment(
            ev,
            &crate::optimizer_v2::CandidateSlot {
                key_len: full_len,
                rank: baseline.full_code_rank(target.word()),
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
            let pattern_consistent = target.full_code().as_slice().starts_with(code.as_slice());
            let mut best_utility = f64::NEG_INFINITY;
            for rank in 1..=(baseline.group(code).len() + 1).min(MAX_RANK) {
                let breakdown = evaluate_assignment(
                    ev,
                    &crate::optimizer_v2::CandidateSlot {
                        key_len: code.len(),
                        rank,
                        occupant_mass: baseline.total_mass(code),
                    },
                    cost,
                    weights,
                    prior_score(target.word(), code, rank),
                    full_len,
                    pattern_consistent,
                );
                best_utility = best_utility.max(breakdown.total());
            }
            if best_utility.is_finite() {
                scored.push(ScoredAssignment {
                    word: target.word().to_string(),
                    code: code.clone(),
                    full_code: target.full_code().clone(),
                    utility: best_utility,
                    mass,
                    baseline_total,
                });
            }
        }
    }

    // 阶段 2:确定性全序(绝对效用降序 → 质量降序 → 词 → 码),状态依赖接纳。
    scored.sort_by(|a, b| {
        b.utility
            .total_cmp(&a.utility)
            .then(b.mass.total_cmp(&a.mass))
            .then(a.word.cmp(&b.word))
            .then(a.code.cmp(&b.code))
    });

    let mut assigned: BTreeSet<String> = BTreeSet::new();
    // 码 → 已分配 v2 词 (质量, 词)(最终次序末尾重排)。
    let mut code_members: BTreeMap<KeySequence, Vec<(f64, String)>> = BTreeMap::new();
    let mut chosen: BTreeMap<String, (ScoredAssignment, UtilityBreakdownV2, f64)> = BTreeMap::new();
    for candidate in scored {
        if assigned.contains(&candidate.word) {
            continue;
        }
        let members = code_members.entry(candidate.code.clone()).or_default();
        let position = merged_position(
            baseline.group(&candidate.code),
            members,
            candidate.mass,
            &candidate.word,
        );
        if position > MAX_RANK {
            continue;
        }
        // 候选位不回退守卫(与 v1 ZERO_REGRESSION 同源的产品约束):分配
        // 不得把词的可达候选位排到其全码真实 rank 之后 —— 否则回放按最小
        // 期望成本选路时,rank≥2 的便宜简码会挤掉全码 rank1 方案,首选
        // 命中崩塌(2026-10 实测 rank1 回退 11~22pp 的根因之一)。
        if position > baseline.full_code_rank(&candidate.word) {
            continue;
        }
        // 分配时点的真实占用质量:baseline + 已分配 v2 词(保守上界)。
        let v2_mass: f64 = members.iter().map(|(m, _)| *m).sum();
        let occupant_mass = baseline.total_mass(&candidate.code) + v2_mass;
        let Some(ev) = evidence.get(&candidate.word) else {
            continue;
        };
        let pattern_consistent = candidate
            .full_code
            .as_slice()
            .starts_with(candidate.code.as_slice());
        let breakdown = evaluate_assignment(
            ev,
            &crate::optimizer_v2::CandidateSlot {
                key_len: candidate.code.len(),
                rank: position,
                occupant_mass,
            },
            cost,
            weights,
            prior_score(&candidate.word, &candidate.code, position),
            candidate.full_code.len(),
            pattern_consistent,
        );
        let net_utility = breakdown.total() - candidate.baseline_total;
        if net_utility <= 0.0 {
            continue; // 词的其他候选码仍有机会(不标记 assigned)。
        }
        assigned.insert(candidate.word.clone());
        members.push((candidate.mass, candidate.word.clone()));
        chosen.insert(candidate.word.clone(), (candidate, breakdown, net_utility));
    }

    // 最终 rank 回填:全部分配完成后按码内质量次序重排。
    let mut entries = BTreeMap::new();
    for (word, (candidate, breakdown, net_utility)) in chosen {
        let members = &code_members[&candidate.code];
        let rank = merged_position(
            baseline.group(&candidate.code),
            members,
            candidate.mass,
            &word,
        );
        entries.insert(
            word.clone(),
            MappingV2Entry {
                word,
                code: candidate.code,
                rank,
                net_utility,
                breakdown,
            },
        );
    }
    MappingV2 { entries }
}

/// 候选在码内合并次序中的 1 起始位次:baseline 质量 + v2 成员按质量降序
/// 混排(同分 baseline 在前;v2 词同分按词形升序)。
fn merged_position(baseline: &[f64], members: &[(f64, String)], mass: f64, word: &str) -> usize {
    let before_baseline = baseline
        .iter()
        .filter(|m| m.total_cmp(&mass).is_gt())
        .count();
    let before_v2 = members
        .iter()
        .filter(|(m, w)| {
            m.total_cmp(&mass).is_gt() || (m.total_cmp(&mass).is_eq() && w.as_str() < word)
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

    /// 锚点 = 1e-4:我们 (p=1e-4) 质量 ln2≈0.693,时间 (8e-5) ≈0.588。
    fn test_scale() -> MassScale {
        MassScale::for_test(1e-4, 1.0, 1.0, 1.0)
    }

    fn empty_baseline() -> BaselineMassView {
        BaselineMassView::for_test(Vec::new(), Vec::new())
    }

    fn two_words() -> (Vec<WordTarget>, BTreeMap<String, LexicalEvidence>) {
        let targets = vec![
            target("我们", "womf", &["wm", "wom"]),
            target("时间", "uijm", &["uj", "uij"]),
        ];
        let evidence = evidence_map(vec![
            word_evidence("我们", "womf", 1e-4),
            word_evidence("时间", "uijm", 8e-5),
        ]);
        (targets, evidence)
    }

    #[test]
    fn determinism_byte_identical() {
        let (targets, evidence) = two_words();
        let run = || {
            produce_mapping(
                &targets,
                &evidence,
                &test_scale(),
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
        // 空码上两词竞争:质量高者 rank 1,另一词占用 rank 2(v2 与 v1
        // 码位唯一的核心区别);扰动定价 = 先占词的质量(已进入净效用)。
        // 时间 的全码真实 rank 为 2 → rank 2 简码不违反候选位不回退守卫。
        let targets = vec![
            target("我们", "womf", &["wm"]),
            target("时间", "uijm", &["wm"]),
        ];
        let evidence = evidence_map(vec![
            word_evidence("我们", "womf", 1e-4),
            word_evidence("时间", "uijm", 8e-5),
        ]);
        let baseline = BaselineMassView::for_test(Vec::new(), vec![("时间", 2)]);
        let mapping = produce_mapping(
            &targets,
            &evidence,
            &test_scale(),
            &baseline,
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
    fn rank_nonregression_guard_rejects_demotion() {
        // 候选位不回退守卫:全码 rank 1 的词不得被分配到 rank ≥ 2 的槽位,
        // 即使净效用为正(防止便宜简码挤掉全码首选)。
        let targets = vec![
            target("我们", "womf", &["wm"]),
            target("时间", "uijm", &["wm"]),
        ];
        let evidence = evidence_map(vec![
            word_evidence("我们", "womf", 1e-4),
            word_evidence("时间", "uijm", 8e-5),
        ]);
        // 两词全码 rank 均为 1(默认)→ 后到的词不能接受 rank 2。
        let mapping = produce_mapping(
            &targets,
            &evidence,
            &test_scale(),
            &empty_baseline(),
            &CostModelV2::default(),
            &EvidenceWeights::default(),
            None,
        );
        assert_eq!(mapping.get("我们").map(|e| e.rank), Some(1));
        assert!(
            mapping.get("时间").is_none(),
            "全码 rank 1 的词不得降到 rank 2"
        );
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
            &test_scale(),
            &empty_baseline(),
            &cost,
            &EvidenceWeights::default(),
            None,
        );
        assert!(mapping.is_empty(), "净效用 ≤ 0 的分配必须丢弃");
    }

    #[test]
    fn baseline_occupants_push_rank_down() {
        // baseline 占用质量(1.0)高于新词(0.693)→ 新词排其后 rank 2
        // (该词全码 rank 为 2,守卫放行)。
        let code: KeySequence = "wm".parse().unwrap();
        let baseline = BaselineMassView::for_test(vec![(code, vec![1.0])], vec![("我们", 2)]);
        let targets = vec![target("我们", "womf", &["wm"])];
        let evidence = evidence_map(vec![word_evidence("我们", "womf", 1e-4)]);
        let mapping = produce_mapping(
            &targets,
            &evidence,
            &test_scale(),
            &baseline,
            &CostModelV2::default(),
            &EvidenceWeights::default(),
            None,
        );
        let entry = mapping.get("我们").expect("我们 应被分配");
        assert_eq!(entry.rank, 2, "baseline 占用者应占据 rank 1");
    }

    #[test]
    fn heavy_occupant_rejects_crowding() {
        // 质量尺度修复的回归锚点:重占用码(质量 5.0)的扰动/歧义定价
        // 必须真实进入净效用并拒绝挤占(修复前 occupant_mass≈1e-5,
        // disruption/ambiguity 系数结构性失效)。全码 rank 2 放行守卫,
        // 由定价决定拒绝。
        let code: KeySequence = "wm".parse().unwrap();
        let baseline = BaselineMassView::for_test(vec![(code, vec![5.0])], vec![("我们", 2)]);
        let targets = vec![target("我们", "womf", &["wm"])];
        let evidence = evidence_map(vec![word_evidence("我们", "womf", 1e-4)]);
        let mapping = produce_mapping(
            &targets,
            &evidence,
            &test_scale(),
            &baseline,
            &CostModelV2::default(),
            &EvidenceWeights::default(),
            None,
        );
        assert!(mapping.is_empty(), "重占用码的挤占应被扰动定价拒绝");
    }

    #[test]
    fn disruption_coeff_changes_decision() {
        // 参数敏感性:同一中度占用码,disruption 0.5 接纳、4.0 拒绝。
        let code: KeySequence = "wm".parse().unwrap();
        let baseline = BaselineMassView::for_test(vec![(code, vec![1.5])], vec![("我们", 2)]);
        let targets = vec![target("我们", "womf", &["wm"])];
        let evidence = evidence_map(vec![word_evidence("我们", "womf", 1e-4)]);
        let with = |disruption_coeff: f64| {
            produce_mapping(
                &targets,
                &evidence,
                &test_scale(),
                &baseline,
                &CostModelV2 {
                    disruption_coeff,
                    ..CostModelV2::default()
                },
                &EvidenceWeights::default(),
                None,
            )
            .len()
        };
        assert_eq!(with(0.5), 1, "低扰动系数应接纳");
        assert_eq!(with(4.0), 0, "高扰动系数应拒绝");
    }
}
