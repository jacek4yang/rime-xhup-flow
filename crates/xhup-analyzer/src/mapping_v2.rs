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
//! 2. **状态依赖接纳**(动态):按短名单顺序遍历;每条按**当时**状态
//!    (baseline + 已分配 v2 词)做码内质量混排分析:位次 = 实际候选位,
//!    选择成本按码内总占用质量(竞争强度)放大,扰动成本只计**实际被挤
//!    后**的候选质量(位次在该词之后的占用者;2026-10 高频词简码丢失
//!    根因 —— 修复前扰动按总质量计,「没挤任何人也要付扰动费」,全码
//!    首选的超高频词反而争不过全码次选词)。**候选位不回退守卫**:分配
//!    位次不得差于该词全码的真实组内 rank(否则回放按最小期望成本选路
//!    时,rank≥2 的便宜简码会挤掉全码 rank1 方案,首选命中崩塌);净
//!    效用 = 该槽位效用 − 该词「留在全码真实 rank」的对照效用;净效用
//!    ≤ 0 或位次越界则跳过(词的其他候选码仍有机会)。
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

    /// 词域全局频率中位锚点(传统保底的 top 段阈值用)。
    pub fn word_median(&self) -> f64 {
        self.word_ref
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
                    | CandidateSource::PrimaryWordShortcut
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
                | CandidateSource::PrimaryWordShortcut
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
    /// 逐字投影模式(如 `FI`;explain 理由卡用)。
    pattern: String,
    /// 绝对效用(跨词竞争排序键;含频率项)。
    utility: f64,
    /// 词的多信号质量(码内位次与占用质量)。
    mass: f64,
    /// 「留在全码真实 rank」的对照效用(每词一次,随记录携带)。
    baseline_total: f64,
    /// 对照效用的分解(explain 用)。
    baseline_breakdown: UtilityBreakdownV2,
}

/// 单个候选码在决策时点的判定(explain)。
#[derive(Clone, Debug)]
pub struct ExplainCandidate {
    /// 候选码。
    pub code: String,
    /// 逐字投影模式(如 `FI`)。
    pub pattern: String,
    /// 决策时点的码内占用质量列表(baseline 候选在前 + 已分配 v2 词,
    /// 各自组内次序;explain 时点快照)。
    pub occupant_masses: Vec<f64>,
    /// 质量混排位次(0 = 未评估:词已由更早候选码分配)。
    pub position: usize,
    /// 效用分解(None = 未到达效用评估:守卫/位次/跳过)。
    pub breakdown: Option<UtilityBreakdownV2>,
    /// 净效用(相对全码对照)。
    pub net_utility: Option<f64>,
    /// 判定。
    pub verdict: ExplainVerdict,
}

/// 候选判定。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplainVerdict {
    /// 接纳。
    Accepted,
    /// 净效用 ≤ 0。
    RejectedNetUtility,
    /// 候选位不回退守卫(位次差于全码真实 rank)。
    RejectedGuard,
    /// 位次超出候选位上限。
    RejectedMaxRank,
    /// 词已由更早的候选码分配(未评估)。
    SkippedWordAssigned,
    /// 传统保底:常规贪心未接纳,按 canonical 生产别名尾部追加保留
    /// (零扰动;回放期望成本不低于全码时首选命中不变)。
    AcceptedTraditionFallback,
}

impl ExplainVerdict {
    /// 报告用稳定标签。
    pub fn label(self) -> &'static str {
        match self {
            ExplainVerdict::Accepted => "accepted",
            ExplainVerdict::RejectedNetUtility => "rejected: 净效用≤0",
            ExplainVerdict::RejectedGuard => "rejected: 候选位不回退守卫",
            ExplainVerdict::RejectedMaxRank => "rejected: 位次超出上限",
            ExplainVerdict::SkippedWordAssigned => "skipped: 词已分配",
            ExplainVerdict::AcceptedTraditionFallback => "accepted: 传统保底(尾部追加)",
        }
    }
}

/// 单词级决策解释( docs/optimizer-v2.md §6 理由卡 + 逐候选拒绝原因)。
#[derive(Clone, Debug)]
pub struct ExplainReport {
    /// 词语。
    pub word: String,
    /// 全码。
    pub full_code: String,
    /// 全码真实词域 rank。
    pub full_code_rank: usize,
    /// 多信号质量。
    pub mass: f64,
    /// 「留在全码」对照效用。
    pub baseline_total: f64,
    /// 「留在全码」对照的效用分解。
    pub baseline_breakdown: UtilityBreakdownV2,
    /// 逐候选判定(决策顺序 = 短名单序)。
    pub candidates: Vec<ExplainCandidate>,
    /// 最终分配 (码, rank);None = 未分配。
    pub outcome: Option<(String, usize)>,
}

/// 渲染人类可读解释报告(确定性文本)。
pub fn render_explain(report: &ExplainReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "词: {}\t全码: {}\t全码rank: {}\t质量: {:.4}\t全码对照效用: {:.4}\n",
        report.word, report.full_code, report.full_code_rank, report.mass, report.baseline_total
    ));
    let b = &report.baseline_breakdown;
    out.push_str(&format!(
        "全码对照分解(频率/省键/先验/选择/扰动/击键/长尾): {:.3}/{:.1}/{:.3}/{:.3}/{:.3}/{:.1}/{:.3}
",
        b.frequency_utility,
        b.keystrokes_saved,
        b.xhup_prior,
        b.selection_cost,
        b.disruption_cost,
        b.keystroke_cost,
        b.rare_pollution
    ));
    match &report.outcome {
        Some((code, rank)) => out.push_str(&format!("最终: 分配 {code} rank {rank}\n")),
        None => out.push_str("最终: 未分配(留全码)\n"),
    }
    out.push_str("候选\t模式\t位次\t占用质量\t净效用\t判定\t主导项\t分解(频率/省键/先验/选择/扰动/击键/长尾)\n");
    for c in &report.candidates {
        let occupants = if c.occupant_masses.is_empty() {
            "空".to_string()
        } else {
            c.occupant_masses
                .iter()
                .map(|m| format!("{m:.2}"))
                .collect::<Vec<_>>()
                .join(",")
        };
        let (net, dominant, detail) = match (c.net_utility, &c.breakdown) {
            (Some(net), Some(b)) => (
                format!("{net:.4}"),
                dominant_term(b).to_string(),
                format!(
                    "{:.3}/{:.1}/{:.3}/{:.3}/{:.3}/{:.1}/{:.3}",
                    b.frequency_utility,
                    b.keystrokes_saved,
                    b.xhup_prior,
                    b.selection_cost,
                    b.disruption_cost,
                    b.keystroke_cost,
                    b.rare_pollution
                ),
            ),
            _ => ("-".to_string(), "-".to_string(), "-".to_string()),
        };
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            c.code,
            c.pattern,
            c.position,
            occupants,
            net,
            c.verdict.label(),
            dominant,
            detail,
        ));
    }
    out
}

/// 确定性贪心分配:词 → (码, 预期 rank)。
///
/// - `targets`:候选词集合(candidates.rs 管线产出);
/// - `evidence`:词 → 多信号词汇证据(缺失词不参与);
/// - `scale`:多信号质量尺度(中位锚点);
/// - `baseline`:固定层码位占用质量视图;
/// - `prior`:XHUP 风格先验(无参考数据传 `None`,全部中立 0.5);
/// - `tradition`:canonical 生产简码层的 词 → 传统码(v2 是这些层的
///   替换候选;传统保底机制的输入,见模块文档)。
#[allow(clippy::too_many_arguments)]
pub fn produce_mapping(
    targets: &[WordTarget],
    evidence: &BTreeMap<String, LexicalEvidence>,
    scale: &MassScale,
    baseline: &BaselineMassView,
    cost: &CostModelV2,
    weights: &EvidenceWeights,
    prior: Option<&XhupStylePrior>,
    tradition: &BTreeMap<String, KeySequence>,
) -> MappingV2 {
    produce_mapping_explained(
        targets,
        evidence,
        scale,
        baseline,
        cost,
        weights,
        prior,
        tradition,
        &[],
    )
    .0
}

/// 同 [`produce_mapping`],额外对 `explain_words` 中的词产出决策解释
/// (决策时点占用快照 + 逐候选效用分解与判定;不改变分配行为)。
#[allow(clippy::too_many_arguments)]
pub fn produce_mapping_explained(
    targets: &[WordTarget],
    evidence: &BTreeMap<String, LexicalEvidence>,
    scale: &MassScale,
    baseline: &BaselineMassView,
    cost: &CostModelV2,
    weights: &EvidenceWeights,
    prior: Option<&XhupStylePrior>,
    tradition: &BTreeMap<String, KeySequence>,
    explain_words: &[&str],
) -> (MappingV2, BTreeMap<String, ExplainReport>) {
    let explain: BTreeSet<&str> = explain_words.iter().copied().collect();
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
        let baseline_breakdown = evaluate_assignment(
            ev,
            &crate::optimizer_v2::CandidateSlot {
                key_len: full_len,
                rank: baseline.full_code_rank(target.word()),
                occupant_mass: 0.0,
                displaced_mass: 0.0,
            },
            cost,
            weights,
            prior_score(target.word(), target.full_code(), 1),
            full_len,
            true,
        );
        // 全码对照不含先验项:v2 只**增加**简码别名,全码入口在两种选择下
        // 都保留,其传统命中是恒定项,不参与对照(2026-10 根因:参考映射对
        // 词只有全码层,先验给全码 +1、给简码 0.3,系统性拒绝高频词简码)。
        let baseline_total = baseline_breakdown.total() - baseline_breakdown.xhup_prior;

        for candidate in target.candidates() {
            let code = candidate.shortcut_code();
            let pattern_consistent = target.full_code().as_slice().starts_with(code.as_slice());
            let (position, displaced, total) =
                slot_analysis(baseline.group(code), &[], mass, target.word());
            let breakdown = evaluate_assignment(
                ev,
                &crate::optimizer_v2::CandidateSlot {
                    key_len: code.len(),
                    rank: position,
                    occupant_mass: total,
                    displaced_mass: displaced,
                },
                cost,
                weights,
                prior_score(target.word(), code, position),
                full_len,
                pattern_consistent,
            );
            scored.push(ScoredAssignment {
                word: target.word().to_string(),
                code: code.clone(),
                full_code: target.full_code().clone(),
                pattern: candidate.mode().pattern(),
                utility: breakdown.total(),
                mass,
                baseline_total,
                baseline_breakdown,
            });
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
    let mut reports: BTreeMap<String, ExplainReport> = BTreeMap::new();
    for candidate in scored {
        let explain_this = explain.contains(candidate.word.as_str());
        if explain_this {
            reports
                .entry(candidate.word.clone())
                .or_insert_with(|| ExplainReport {
                    word: candidate.word.clone(),
                    full_code: candidate.full_code.to_string(),
                    full_code_rank: baseline.full_code_rank(&candidate.word),
                    mass: candidate.mass,
                    baseline_total: candidate.baseline_total,
                    baseline_breakdown: candidate.baseline_breakdown,
                    candidates: Vec::new(),
                    outcome: None,
                });
        }
        if assigned.contains(&candidate.word) {
            if explain_this {
                let report = reports.get_mut(&candidate.word).expect("报告已建");
                report.candidates.push(ExplainCandidate {
                    code: candidate.code.to_string(),
                    pattern: candidate.pattern.clone(),
                    occupant_masses: Vec::new(),
                    position: 0,
                    breakdown: None,
                    net_utility: None,
                    verdict: ExplainVerdict::SkippedWordAssigned,
                });
            }
            continue;
        }
        let members = code_members.entry(candidate.code.clone()).or_default();
        let (position, displaced, total) = slot_analysis(
            baseline.group(&candidate.code),
            members,
            candidate.mass,
            &candidate.word,
        );
        // 决策时点占用快照(baseline 组内次序 + 已分配 v2 词记录序)。
        let occupant_snapshot = |members: &Vec<(f64, String)>| -> Vec<f64> {
            baseline
                .group(&candidate.code)
                .iter()
                .copied()
                .chain(members.iter().map(|(m, _)| *m))
                .collect()
        };
        if position > MAX_RANK {
            if explain_this {
                let report = reports.get_mut(&candidate.word).expect("报告已建");
                report.candidates.push(ExplainCandidate {
                    code: candidate.code.to_string(),
                    pattern: candidate.pattern.clone(),
                    occupant_masses: occupant_snapshot(members),
                    position,
                    breakdown: None,
                    net_utility: None,
                    verdict: ExplainVerdict::RejectedMaxRank,
                });
            }
            continue;
        }
        // 候选位不回退守卫(精确版):只有当简码方案会在回放选路中**真正
        // 挤掉**更优的全码方案(位次更差且期望成本严格更低)时才拒绝;
        // 位次差但不更便宜的别名不影响首选命中 —— canonical FF 层「追加
        // 在 baseline 之后」正是此形态(2026-10 修复前守卫过宽,把
        // 传统尾部别名一并挡掉)。
        let full_rank = baseline.full_code_rank(&candidate.word);
        if position > full_rank {
            let replay_rank_cost =
                |rank: usize| cost.rank_cost[rank.saturating_sub(1).min(cost.rank_cost.len() - 1)];
            let slot_cost =
                candidate.code.len() as f64 * cost.key_cost + replay_rank_cost(position);
            let full_cost =
                candidate.full_code.len() as f64 * cost.key_cost + replay_rank_cost(full_rank);
            if slot_cost < full_cost {
                if explain_this {
                    let report = reports.get_mut(&candidate.word).expect("报告已建");
                    report.candidates.push(ExplainCandidate {
                        code: candidate.code.to_string(),
                        pattern: candidate.pattern.clone(),
                        occupant_masses: occupant_snapshot(members),
                        position,
                        breakdown: None,
                        net_utility: None,
                        verdict: ExplainVerdict::RejectedGuard,
                    });
                }
                continue;
            }
        }
        // 分配时点的占用质量拆分:竞争强度 = 码内总质量(放大选择成本);
        // 扰动 = 实际被挤后的候选质量(slot_analysis 已按位次拆分)。
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
                occupant_mass: total,
                displaced_mass: displaced,
            },
            cost,
            weights,
            prior_score(&candidate.word, &candidate.code, position),
            candidate.full_code.len(),
            pattern_consistent,
        );
        let net_utility = breakdown.total() - candidate.baseline_total;
        if net_utility <= 0.0 {
            if explain_this {
                let report = reports.get_mut(&candidate.word).expect("报告已建");
                report.candidates.push(ExplainCandidate {
                    code: candidate.code.to_string(),
                    pattern: candidate.pattern.clone(),
                    occupant_masses: occupant_snapshot(members),
                    position,
                    breakdown: Some(breakdown),
                    net_utility: Some(net_utility),
                    verdict: ExplainVerdict::RejectedNetUtility,
                });
            }
            continue; // 词的其他候选码仍有机会(不标记 assigned)。
        }
        if explain_this {
            let report = reports.get_mut(&candidate.word).expect("报告已建");
            report.candidates.push(ExplainCandidate {
                code: candidate.code.to_string(),
                pattern: candidate.pattern.clone(),
                occupant_masses: occupant_snapshot(members),
                position,
                breakdown: Some(breakdown),
                net_utility: Some(net_utility),
                verdict: ExplainVerdict::Accepted,
            });
        }
        assigned.insert(candidate.word.clone());
        members.push((candidate.mass, candidate.word.clone()));
        chosen.insert(candidate.word.clone(), (candidate, breakdown, net_utility));
    }

    // 最终 rank 回填:全部分配完成后按码内质量次序重排。
    let mut entries: BTreeMap<String, MappingV2Entry> = BTreeMap::new();
    for (word, (candidate, breakdown, net_utility)) in chosen {
        let members = &code_members[&candidate.code];
        let rank = slot_analysis(
            baseline.group(&candidate.code),
            members,
            candidate.mass,
            &word,
        )
        .0;
        if let Some(report) = reports.get_mut(&word) {
            report.outcome = Some((candidate.code.to_string(), rank));
        }
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
    // 传统保底(高频词简码丢失 P0 的结构性修复):持有 canonical 生产
    // 简码别名(tradition)且频率在 top 段(≥ 词域中位锚点)的词,若常规
    // 贪心未接纳任何候选,在其传统码**尾部追加**保留别名 —— 零扰动
    // (不挤任何候选),回放期望成本不低于全码时首选命中不变,肌肉记忆
    // 静默丢失被制度性杜绝。偏离传统仍需证据:词若已被贪心分配到其他
    // 码,保底不触发。
    for (word, code) in tradition {
        if entries.contains_key(word) {
            continue;
        }
        let Some(ev) = evidence.get(word) else {
            continue;
        };
        if ev.normalized_frequency() < scale.word_median() {
            continue; // top 段以外:常规规则,允许淘汰
        }
        let members = code_members.get(code).map(Vec::as_slice).unwrap_or(&[]);
        let append_rank = baseline.group(code).len() + members.len() + 1;
        let full_len = ev.code().len();
        let baseline_total = evaluate_assignment(
            ev,
            &crate::optimizer_v2::CandidateSlot {
                key_len: full_len,
                rank: baseline.full_code_rank(word),
                occupant_mass: 0.0,
                displaced_mass: 0.0,
            },
            cost,
            weights,
            prior_score(word, ev.code(), 1),
            full_len,
            true,
        );
        let breakdown = evaluate_assignment(
            ev,
            &crate::optimizer_v2::CandidateSlot {
                key_len: code.len(),
                rank: append_rank,
                occupant_mass: baseline.total_mass(code)
                    + members.iter().map(|(m, _)| *m).sum::<f64>(),
                displaced_mass: 0.0,
            },
            cost,
            weights,
            prior_score(word, code, append_rank),
            full_len,
            ev.code().as_slice().starts_with(code.as_slice()),
        );
        let net_utility = breakdown.total() - (baseline_total.total() - baseline_total.xhup_prior);
        if let Some(report) = reports.get_mut(word) {
            report.candidates.push(ExplainCandidate {
                code: code.to_string(),
                pattern: "tradition".to_string(),
                occupant_masses: baseline
                    .group(code)
                    .iter()
                    .copied()
                    .chain(members.iter().map(|(m, _)| *m))
                    .collect(),
                position: append_rank,
                breakdown: Some(breakdown),
                net_utility: Some(net_utility),
                verdict: ExplainVerdict::AcceptedTraditionFallback,
            });
            report.outcome = Some((code.to_string(), append_rank));
        }
        entries.insert(
            word.clone(),
            MappingV2Entry {
                word: word.clone(),
                code: code.clone(),
                rank: append_rank,
                net_utility,
                breakdown,
            },
        );
    }
    (MappingV2 { entries }, reports)
}

/// 码内质量混排分析:(1 起始位次, 被挤后质量合计, 占用总质量)。
///
/// 次序规则:质量降序;同分 baseline 候选在前、v2 词按词形升序(确定性
/// 兜底)。位次 ≤ 该词质量位次的候选不被挤动;只有位次之后的候选计入
/// displaced(扰动成本只计真实被挤者 —— 高频词简码丢失根因修复:
/// 修复前扰动按码内总质量计,「没挤任何人也要付扰动费」)。
fn slot_analysis(
    baseline: &[f64],
    members: &[(f64, String)],
    mass: f64,
    word: &str,
) -> (usize, f64, f64) {
    let mut before = 0usize;
    let mut displaced = 0.0;
    let mut total = 0.0;
    for m in baseline {
        total += *m;
        if m.total_cmp(&mass).is_gt() {
            before += 1;
        } else {
            displaced += *m;
        }
    }
    for (m, w) in members {
        total += *m;
        if m.total_cmp(&mass).is_gt() || (m.total_cmp(&mass).is_eq() && w.as_str() < word) {
            before += 1;
        } else {
            displaced += *m;
        }
    }
    (before + 1, displaced, total)
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
                &BTreeMap::new(),
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
            &BTreeMap::new(),
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
            &BTreeMap::new(),
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
            &BTreeMap::new(),
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
            &BTreeMap::new(),
        );
        let entry = mapping.get("我们").expect("我们 应被分配");
        assert_eq!(entry.rank, 2, "baseline 占用者应占据 rank 1");
    }

    #[test]
    fn heavy_occupant_rejects_crowding() {
        // 扰动定价只计真实被挤者:词质量 ln1p(100)≈4.62 > 占用者 3.0 →
        // 占用者被挤后,disruption 生效并拒绝(全码 rank 2 放行守卫,
        // 由定价决定拒绝)。
        let code: KeySequence = "wm".parse().unwrap();
        let baseline = BaselineMassView::for_test(vec![(code, vec![3.0])], vec![("我们", 2)]);
        let targets = vec![target("我们", "womf", &["wm"])];
        let evidence = evidence_map(vec![word_evidence("我们", "womf", 1e-2)]);
        let mapping = produce_mapping(
            &targets,
            &evidence,
            &test_scale(),
            &baseline,
            &CostModelV2::default(),
            &EvidenceWeights::default(),
            None,
            &BTreeMap::new(),
        );
        assert!(mapping.is_empty(), "挤动重占用者应被扰动定价拒绝");
    }

    #[test]
    fn tailgating_light_word_pays_only_selection() {
        // 跟在高占用者**之后**(不挤任何人)只付选择成本、不付扰动成本 —
        // 占用/扰动拆分语义的对称锚点(2026-10 修复前此情形被误收扰动费,
        // 是高频词简码丢失的根因)。
        let code: KeySequence = "wm".parse().unwrap();
        let baseline = BaselineMassView::for_test(vec![(code, vec![3.0])], vec![("我们", 2)]);
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
            &BTreeMap::new(),
        );
        let entry = mapping.get("我们").expect("跟排不挤人,应被接纳");
        assert_eq!(entry.rank, 2);
        assert_eq!(
            entry.breakdown.disruption_cost, 0.0,
            "未被挤者不产生扰动成本"
        );
        assert!(
            entry.breakdown.selection_cost > 0.0,
            "竞争强度仍放大选择成本"
        );
    }

    #[test]
    fn disruption_coeff_changes_decision() {
        // 参数敏感性:词挤动中度占用者(3.0),disruption 0.5 接纳、2.0 拒绝。
        let code: KeySequence = "wm".parse().unwrap();
        let baseline = BaselineMassView::for_test(vec![(code, vec![3.0])], vec![("我们", 2)]);
        let targets = vec![target("我们", "womf", &["wm"])];
        let evidence = evidence_map(vec![word_evidence("我们", "womf", 1e-2)]);
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
                &BTreeMap::new(),
            )
            .len()
        };
        assert_eq!(with(0.5), 1, "低扰动系数应接纳");
        assert_eq!(with(2.0), 0, "高扰动系数应拒绝");
    }

    #[test]
    fn full_rank1_word_wins_scarce_slot_over_lower_rank_word() {
        // 「就是 vs 九十 争 jqu」型回归锚点(2026-10 高频词简码丢失):
        // 全码 rank1 的高频词在轻占用码上必须拿到 rank1;修复前扰动按码内
        // 总质量计费 → 该词净效用为负被拒,码位被全码 rank2 的低频词拿走。
        let code: KeySequence = "ab".parse().unwrap();
        let baseline =
            BaselineMassView::for_test(vec![(code, vec![0.6, 0.95])], vec![("甲", 1), ("乙", 2)]);
        let targets = vec![target("甲", "abcd", &["ab"]), target("乙", "abce", &["ab"])];
        let evidence = evidence_map(vec![
            word_evidence("甲", "abcd", 1e-2), // 质量 ln1p(100) ≈ 4.62
            word_evidence("乙", "abce", 1e-3), // 质量 ln1p(10) ≈ 2.40
        ]);
        let mapping = produce_mapping(
            &targets,
            &evidence,
            &test_scale(),
            &baseline,
            &CostModelV2 {
                rank_cost: [0.0, 1.0, 2.0, 4.0],
                ambiguity_coeff: 0.25,
                disruption_coeff: 0.5,
                ..CostModelV2::default()
            },
            &EvidenceWeights::default(),
            None,
            &BTreeMap::new(),
        );
        let jia = mapping.get("甲").expect("全码 rank1 高频词必须拿到简码");
        assert_eq!(jia.code, "ab".parse().unwrap());
        assert_eq!(jia.rank, 1, "稀缺位必须归全码 rank1 的高频词");
        // 乙 不被挤到自身全码 rank 之后:rank 2 接纳或留全码均可。
        if let Some(yi) = mapping.get("乙") {
            assert_eq!(yi.rank, 2);
        }
    }

    #[test]
    fn explain_reports_verdicts_and_state() {
        // 两词竞争同一空码,时间 全码 rank 2:我们 先拿 wm rank1,
        // 时间 的 wm 候选在决策时点应看到 我们 的占用质量并 accepted rank2;
        // 时间 的 uj 候选(被 baseline 占用压低了短名单序)随后 skipped。
        let targets = vec![
            target("我们", "womf", &["wm"]),
            target("时间", "uijm", &["wm", "uj"]),
        ];
        let evidence = evidence_map(vec![
            word_evidence("我们", "womf", 1e-4),
            word_evidence("时间", "uijm", 8e-5),
        ]);
        let baseline =
            BaselineMassView::for_test(vec![("uj".parse().unwrap(), vec![1.0])], vec![("时间", 2)]);
        let (mapping, reports) = produce_mapping_explained(
            &targets,
            &evidence,
            &test_scale(),
            &baseline,
            &CostModelV2::default(),
            &EvidenceWeights::default(),
            None,
            &BTreeMap::new(),
            &["时间"],
        );
        let report = reports.get("时间").expect("应有解释报告");
        assert_eq!(report.full_code_rank, 2);
        assert!(report.mass > 0.0);
        let wm = report
            .candidates
            .iter()
            .find(|c| c.code == "wm")
            .expect("wm 候选必有判定");
        assert_eq!(wm.verdict, ExplainVerdict::Accepted);
        assert_eq!(wm.position, 2);
        assert!(
            wm.occupant_masses.iter().any(|m| *m > 0.0),
            "决策时点应看到先分配的 我们 的占用质量"
        );
        assert!(wm.net_utility.expect("已评估必有净效用") > 0.0);
        // 词已分配后,后续候选标记 skipped。
        let uj = report
            .candidates
            .iter()
            .find(|c| c.code == "uj")
            .expect("uj 候选必有判定");
        assert_eq!(uj.verdict, ExplainVerdict::SkippedWordAssigned);
        assert_eq!(report.outcome, Some(("wm".to_string(), 2)));
        assert_eq!(mapping.get("时间").map(|e| e.rank), Some(2));
        // 渲染确定性且包含关键列。
        let text = render_explain(report);
        assert!(text.contains("wm") && text.contains("accepted"));
    }

    #[test]
    fn explain_guard_rejection_is_visible() {
        // 守卫拒绝必须在解释中可见(不是静默消失)。
        let targets = vec![
            target("我们", "womf", &["wm"]),
            target("时间", "uijm", &["wm"]),
        ];
        let evidence = evidence_map(vec![
            word_evidence("我们", "womf", 1e-4),
            word_evidence("时间", "uijm", 8e-5),
        ]);
        // 时间 全码 rank 1(默认)→ rank 2 槽位被守卫拒绝。
        let (_, reports) = produce_mapping_explained(
            &targets,
            &evidence,
            &test_scale(),
            &empty_baseline(),
            &CostModelV2::default(),
            &EvidenceWeights::default(),
            None,
            &BTreeMap::new(),
            &["时间"],
        );
        let report = reports.get("时间").expect("应有解释报告");
        let wm = report.candidates.iter().find(|c| c.code == "wm").unwrap();
        assert_eq!(wm.verdict, ExplainVerdict::RejectedGuard);
        assert_eq!(report.outcome, None);
    }

    #[test]
    fn tradition_fallback_preserves_top_frequency_alias() {
        // 传统保底:词持有 canonical 别名(wm),常规贪心被重占用码拒绝
        // (挤动 3.0 质量),频率 ≥ 中位锚点 → 尾部追加保留,零扰动。
        let code: KeySequence = "wm".parse().unwrap();
        let baseline =
            BaselineMassView::for_test(vec![(code.clone(), vec![3.0])], vec![("我们", 2)]);
        let targets = vec![target("我们", "womf", &["wm"])];
        let evidence = evidence_map(vec![word_evidence("我们", "womf", 1e-2)]);
        let mut tradition = BTreeMap::new();
        tradition.insert("我们".to_string(), code);
        let (mapping, reports) = produce_mapping_explained(
            &targets,
            &evidence,
            &test_scale(),
            &baseline,
            &CostModelV2::default(),
            &EvidenceWeights::default(),
            None,
            &tradition,
            &["我们"],
        );
        let entry = mapping.get("我们").expect("传统保底必须保留别名");
        assert_eq!(entry.code, "wm".parse().unwrap());
        assert_eq!(entry.rank, 2, "尾部追加:既有占用者之后");
        assert_eq!(entry.breakdown.disruption_cost, 0.0, "保底追加不挤任何候选");
        let report = reports.get("我们").expect("应有解释报告");
        assert!(
            report
                .candidates
                .iter()
                .any(|c| c.verdict == ExplainVerdict::AcceptedTraditionFallback),
            "解释报告必须标明保底来源"
        );
        assert_eq!(report.outcome, Some(("wm".to_string(), 2)));
    }

    #[test]
    fn tradition_fallback_skips_below_median_words() {
        // 频率低于词域中位锚点的词不走保底(允许淘汰),也不占位。
        let code: KeySequence = "wm".parse().unwrap();
        // 全码 rank 1 → 守卫拒绝 rank 2 简码(便宜方案会 demote);
        // 低频 → 保底不触发。
        let baseline =
            BaselineMassView::for_test(vec![(code.clone(), vec![3.0])], vec![("我们", 1)]);
        let targets = vec![target("我们", "womf", &["wm"])];
        // 锚点 1e-4,p = 1e-6 < 锚点 → 非 top 段。
        let evidence = evidence_map(vec![word_evidence("我们", "womf", 1e-6)]);
        let mut tradition = BTreeMap::new();
        tradition.insert("我们".to_string(), code);
        let mapping = produce_mapping(
            &targets,
            &evidence,
            &test_scale(),
            &baseline,
            &CostModelV2::default(),
            &EvidenceWeights::default(),
            None,
            &tradition,
        );
        assert!(mapping.is_empty(), "低频词不走传统保底");
    }
}
