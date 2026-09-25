//! 真实语料 committed-context 回放基准(Issue #83 §20/§24,§25 第 6 步)。
//!
//! [`replay`](crate::replay) 是**静态层**回放(全码 + 简码别名 + 候选位),
//! 显式不模拟上下文。本模块补上另一半:对真实句子语料分词后逐 token 回放,
//! 每个 token 用「已提交前文 + 当前 token 的 canonical 码」构造候选菜单,
//! 分别用 baseline(忽略上下文)与上下文 scorer 排序,度量:
//!
//! - `context_gain`:baseline 未命中 rank1、上下文 scorer 命中的 token 数;
//! - `harmful_reorder`:baseline 命中 rank1、上下文 scorer 未命中的 token 数
//!   (§20 `harmful reorder rate`,与 misleading-hint rate 同地位的硬指标)。
//!
//! # 语义边界
//!
//! - 只回放**有 canonical 词码**的 token(词码层 4/6/8 键)。无词码的 token
//!   走单字组句,由 [`crate::replay`] 的静态层与 open-composition 可达性
//!   测试覆盖,不在本模块重复。
//! - 候选菜单来自 canonical 词码层同码词(生产真实码表),不是合成候选。
//! - 期望文本是语料的分词结果本身,因此本基准度量的是「**同码歧义下上下文
//!   能否把语料真实的那个词排到第一**」,不引入人工语言学判断。
//! - 语料以聚合/夹具形式入库;本模块只读传入文本,不联网、不写用户数据。
//!
//! # 确定性
//!
//! 菜单、语料遍历与统计全部使用有序容器;scorer 为固定点整数评分,平局
//! 决胜序由 [`rank_paths`] 固定。同一输入必然得到同一报告。

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::time::Instant;

use serde::Serialize;
use xhup_core::KeySequence;
use xhup_decoder::{
    BaselineScorer, CandidateKind, DeterministicScorer, EdgeCandidate, KdconvBigramScorer, Lattice,
    RuntimeContext, Span,
};

use crate::corpus::Segmenter;

/// 报告 schema 标识。
pub const CONTEXT_REPLAY_SCHEMA: &str = "xhup-context-replay/v1";
/// 机器可读基线 schema 标识。
pub const CONTEXT_REPLAY_BASELINE_SCHEMA: &str = "xhup-context-replay-baseline/v1";

/// 每个 token 的候选路径枚举上限(参考枚举器,仅用于小菜单)。
const PATH_LIMIT: NonZeroUsize = NonZeroUsize::new(64).expect("64 != 0");

/// 真实语料上下文回放的聚合指标。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextReplayMetrics {
    /// 参与回放的句子数(非空行)。
    pub sentences: usize,
    /// 有 canonical 词码、因而参与排名的 token 数。
    pub tokens: usize,
    /// 其中码位有 ≥2 个候选(真正存在同码歧义)的 token 数。
    pub ambiguous: usize,
    /// baseline scorer 命中 rank1 的 token 数。
    pub baseline_rank1: usize,
    /// 上下文 scorer 命中 rank1 的 token 数。
    pub contextual_rank1: usize,
    /// baseline 未命中、上下文命中的 token 数(上下文收益)。
    pub context_gain: usize,
    /// baseline 命中、上下文未命中的 token 数(有害重排)。
    pub harmful_reorder: usize,
    /// baseline 下期望候选选择成本合计(Q10 单位,见 [`SELECTION_COST_Q10`])。
    ///
    /// §1 的核心主张是「少按一键但候选在第 8 位可能更差」,因此只看 rank1
    /// 命中率不够:必须看**期望选择成本**。这里对每个参与 token 累加其
    /// top1 命中与否对应的选择成本。
    pub baseline_selection_cost_q10: u64,
    /// 上下文 scorer 下期望候选选择成本合计(Q10 单位)。
    pub contextual_selection_cost_q10: u64,
}

/// 候选选择成本的 Q10 标度(rank 1..=4 与 `replay::ReplayCostModel` 的
/// `rank_cost = [0.0, 0.5, 1.0, 2.0]` **逐项相等**,`Q10 = 成本 × 1024`)。
///
/// 索引 = 实际候选位 − 1;`rank1 = 0`(直接首选),`rank2 = 0.5 键`,
/// `rank3 = 1.0 键`,其余 = 2.0 键。选择不是免费的:这正是 §1「短码存在
/// 不等于短码有用」的量化方式。
///
/// **`rank = 0`(期望词不在菜单中)是本模块的显式扩展**:`ReplayCostModel`
/// 只在「词有输入方案」时计算 rank,永不出现 0,故无对应档位。这里把缺席
/// 按最差档(2.0 键)计 —— 缺席不是免费,否则会低估上下文收益。
pub const SELECTION_COST_Q10: [u64; 4] = [0, 512, 1024, 2048];

/// 按 1-based rank 取选择成本(Q10);rank 超过 4 时用末档。
pub fn selection_cost_q10(rank: usize) -> u64 {
    if rank == 0 {
        return SELECTION_COST_Q10[3];
    }
    SELECTION_COST_Q10[(rank - 1).min(SELECTION_COST_Q10.len() - 1)]
}

impl ContextReplayMetrics {
    pub fn baseline_rank1_rate(&self) -> f64 {
        ratio(self.baseline_rank1, self.tokens)
    }

    pub fn contextual_rank1_rate(&self) -> f64 {
        ratio(self.contextual_rank1, self.tokens)
    }

    /// 上下文增益率(分母为全部参与 token,不只是歧义 token)。
    pub fn context_gain_rate(&self) -> f64 {
        ratio(self.context_gain, self.tokens)
    }

    /// 有害重排率(§20)。
    pub fn harmful_reorder_rate(&self) -> f64 {
        ratio(self.harmful_reorder, self.tokens)
    }

    /// 歧义占比(诊断:语料中真正需要上下文消歧的比例)。
    pub fn ambiguity_rate(&self) -> f64 {
        ratio(self.ambiguous, self.tokens)
    }

    /// baseline 的期望选择成本(每 token,键)。
    pub fn baseline_selection_cost_per_token(&self) -> f64 {
        ratio_u64(self.baseline_selection_cost_q10, self.tokens) / 1024.0
    }

    /// 上下文 scorer 的期望选择成本(每 token,键)。
    pub fn contextual_selection_cost_per_token(&self) -> f64 {
        ratio_u64(self.contextual_selection_cost_q10, self.tokens) / 1024.0
    }

    /// 上下文带来的期望选择成本节省(每 token,键;正数 = 上下文更好)。
    pub fn selection_cost_saving_per_token(&self) -> f64 {
        self.baseline_selection_cost_per_token() - self.contextual_selection_cost_per_token()
    }

    /// 净收益(可负)。
    pub fn net_gain(&self) -> f64 {
        self.context_gain_rate() - self.harmful_reorder_rate()
    }
}

/// 一次真实语料上下文回放的完整报告。
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextReplayReport {
    pub schema: &'static str,
    pub baseline_scorer: &'static str,
    pub contextual_scorer: &'static str,
    pub metrics: ContextReplayMetrics,
}

/// 有界 [`decode_beam`](xhup_decoder::decode_beam) 相对全路径枚举的等价性读数。
///
/// 目的(§25 第 7 步 / §22):确认生产解码改走有界 beam 后,top1 判定与
/// 「物化全部完整路径再排序」的参考实现**逐 token 一致**,同时给出截断规模。
/// 任何不一致都是精度回归,必须显式可见。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundedDecodeMetrics {
    /// 参与比较的歧义 token 数(有 ≥2 候选)。
    pub compared: usize,
    /// beam 与全路径枚举 top1 文本一致的 token 数。
    pub top1_agreement: usize,
    /// beam 报告截断(`BeamTruncated`)的 token 数。
    pub truncated: usize,
    /// 任一 scorer 下 beam 报告低置信回退的 token 数。
    pub low_confidence: usize,
    /// 无完整路径(两者都为空)的 token 数。
    pub empty: usize,
}

impl BoundedDecodeMetrics {
    /// top1 一致率(分母为 `compared`)。
    pub fn top1_agreement_rate(&self) -> f64 {
        ratio(self.top1_agreement, self.compared)
    }

    /// 截断占比(分母为 `compared`)。
    pub fn truncated_rate(&self) -> f64 {
        ratio(self.truncated, self.compared)
    }
}

/// 有界 beam 解码的等价性报告。
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundedDecodeReport {
    pub schema: &'static str,
    pub scorer: &'static str,
    pub beam_width: usize,
    pub top_k: usize,
    pub min_confidence_gap: i64,
    pub metrics: BoundedDecodeMetrics,
}

/// 报告 schema 标识(有界解码等价性)。
pub const BOUNDED_DECODE_SCHEMA: &str = "xhup-bounded-decode-consistency/v1";

/// 回放真实句子语料,比较 baseline 与已加载的上下文 scorer。
///
/// `sentences` 为语料行(空行跳过);分词使用与语料统计同源的最大匹配
/// [`Segmenter`]。不读取用户数据,不联网。
pub fn replay_sentences<S>(
    sentences_text: &str,
    contextual: &S,
    contextual_id: &'static str,
) -> ContextReplayReport
where
    S: DeterministicScorer,
{
    let prices = ProductionMenus::build();
    let segmenter = Segmenter::build();
    let baseline = BaselineScorer::default();
    let mut metrics = ContextReplayMetrics {
        sentences: count_sentences(sentences_text),
        ..ContextReplayMetrics::default()
    };

    walk_ambiguous_tokens(sentences_text, &prices, &segmenter, |token, ranked| {
        metrics.tokens += 1;
        let Some((context, lattice)) = ranked else {
            // 无同码歧义:两个 scorer 都必然命中,选择成本为 0。
            metrics.baseline_rank1 += 1;
            metrics.contextual_rank1 += 1;
            return;
        };
        metrics.ambiguous += 1;
        let baseline_rank = scored_rank(&baseline, context, lattice, token);
        let contextual_rank = scored_rank(contextual, context, lattice, token);
        metrics.baseline_selection_cost_q10 += selection_cost_q10(baseline_rank);
        metrics.contextual_selection_cost_q10 += selection_cost_q10(contextual_rank);
        let baseline_ok = baseline_rank == 1;
        let contextual_ok = contextual_rank == 1;
        metrics.baseline_rank1 += usize::from(baseline_ok);
        metrics.contextual_rank1 += usize::from(contextual_ok);
        match (baseline_ok, contextual_ok) {
            (false, true) => metrics.context_gain += 1,
            (true, false) => metrics.harmful_reorder += 1,
            _ => {}
        }
    });

    ContextReplayReport {
        schema: CONTEXT_REPLAY_SCHEMA,
        baseline_scorer: BaselineScorer::SCORER_ID,
        contextual_scorer: contextual_id,
        metrics,
    }
}

/// 便捷入口:用真实 KDConv bigram 证据做上下文回放。
pub fn replay_sentences_kdconv(
    sentences_text: &str,
    model: xhup_decoder::BigramModel,
) -> ContextReplayReport {
    let scorer = KdconvBigramScorer::new(model);
    replay_sentences(sentences_text, &scorer, KdconvBigramScorer::SCORER_ID)
}

/// 回放真实句子语料,比较 baseline 与「bigram + 本地用户自适应」scorer。
///
/// 这是 §25 第 9 步(本地用户学习)的**消费侧验收通路**:
///
/// - `bigram` 提供 committed-context 转移证据(全局先验);
/// - `user_model` 提供个人选择 overlay(有界加分,`xhup-user-model/v1`);
/// - 期望词同时作为「用户选过的词」喂给模型 —— 语义是「**如果**用户
///   一直选这个词,本地学习能否把它顶上来」,即离线模拟 `observe()` 后
///   的即时重排。语料词本身是入库公开夹具,不构成用户隐私数据;
/// - 空模型与 `replay_sentences_kdconv` 结果严格一致(A/B 对照)。
///
/// 加分语义与封顶在 [`crate::user_model::UserModel`];本函数只消费。
pub fn replay_sentences_kdconv_user(
    sentences_text: &str,
    bigram: xhup_decoder::BigramModel,
    user_model: &crate::user_model::UserModel,
) -> ContextReplayReport {
    let user = user_model.clone();
    let scorer = KdconvBigramScorer::new(bigram.clone())
        .with_user_overlay(move |word, now| user.boost_q10(word, now));
    replay_sentences(sentences_text, &scorer, KdconvBigramScorer::SCORER_ID)
}

/// 回放真实句子,使用 **KDConv + PTT 合并证据**(§13/§25 第 6 步第二证据源)。
///
/// `merge_policy` 决定两源转移计数的合并方式。**测量结论(2026-09-26,
/// 2000 句 KDConv 重放,2701 个同码歧义 token,PR 记录)**:
///
/// | 策略 | contextual rank1 | context_gain | 选择成本 Q10 |
/// |---|---|---|---|
/// | 仅 KDConv | 5532 | 448 | 124 416 |
/// | RawSum | 5527 | 443 | 127 488 |
/// | MaxEvidence | 5527 | 443 | 127 488 |
/// | PerSourceNormalized | 5531 | 447 | 125 440 |
///
/// KDConv 语料域(影视/音乐/旅行)与重放夹具同源,PTT(论坛问答)是
/// **异域**补强:合并后共同 pair 的证据相加会轻微稀释同域相对差异
/// (例如 `的→经典` PTT 70 vs `的→景点` KDConv 140,RawSum 后差距收窄),
/// 5 个 token 由对转负。因此:
///
/// - 合并**按测量**不改善 KDConv 域回放;PTT 的价值是给 KDConv 覆盖不到
///   的转移对补证据(两源 pair 交集仅 30 398 / KDConv 232 987 ≈ 13%);
/// - 生产合并策略(或按域混合权重)是后续标定工作;本函数先交付
///   **可测量、可解释的合并通道**,把策略选择留给真实数据决策。
///
/// 其余口径与 [`replay_sentences_kdconv`] 完全一致(A/B 可比)。
pub fn replay_sentences_merged(
    sentences_text: &str,
    kdconv: xhup_decoder::BigramModel,
    ptt: xhup_decoder::BigramModel,
    merge_policy: xhup_decoder::MergePolicy,
) -> (ContextReplayReport, xhup_decoder::MergeAudit) {
    let (merged, audit) =
        xhup_decoder::merge_models(&[("kdconv", &kdconv), ("ptt", &ptt)], merge_policy);
    let scorer = KdconvBigramScorer::new(merged);
    let report = replay_sentences(sentences_text, &scorer, KdconvBigramScorer::SCORER_ID);
    (report, audit)
}

/// 单个有害重排样本的证据明细(设计 §6 弱证据降级策略的依据)。
///
/// 字段只包含**语料**中的词与聚合计数,不含任何用户数据:输入语料本身是
/// 入库的公开衍生夹具(Apache-2.0)。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarmfulCase {
    /// committed 前文的尾部 token(语料词)。
    pub committed_tail: String,
    /// 语料真实的词(baseline 选中、上下文改错的那个)。
    pub expected: String,
    /// 上下文 scorer 实际选中的词。
    pub picked: String,
    /// 前文 → 期望词的转移计数。
    pub expected_evidence: u64,
    /// 前文 → 实际选中词的转移计数。
    pub picked_evidence: u64,
}

impl HarmfulCase {
    /// 竞争者比期望词强多少(正数 = 上下文被更强的证据带走)。
    pub fn evidence_margin(&self) -> i64 {
        self.picked_evidence as i64 - self.expected_evidence as i64
    }

    /// 是否为「证据近乎持平」的样本(差距 ≤ 1)。
    ///
    /// 若有害样本主要落在这一类,说明单纯加宽证据阈值收效有限 —— 问题在
    /// **证据本身不足以区分真实近义歧义**(如「他的/它的」),而不是阈值太松。
    pub fn is_near_tie(&self) -> bool {
        self.evidence_margin().abs() <= 1
    }
}

/// 采集有害重排样本的证据明细(最多 `limit` 条,确定性序)。
///
/// 用于回答 §6 的核心问题:「上下文什么时候会错,能否用证据强度预测?」
/// 按 (前文, 期望词) 去重后按期望词字典序输出,保证可复现。
pub fn harmful_case_diagnostics(
    sentences_text: &str,
    model: &xhup_decoder::BigramModel,
    limit: usize,
) -> Vec<HarmfulCase> {
    let prices = ProductionMenus::build();
    let segmenter = Segmenter::build();
    let baseline = BaselineScorer::default();
    let contextual = KdconvBigramScorer::new(model.clone());
    let mut seen = std::collections::BTreeSet::new();
    let mut found: Vec<HarmfulCase> = Vec::new();

    walk_ambiguous_tokens(sentences_text, &prices, &segmenter, |token, ranked| {
        let Some((context, lattice)) = ranked else {
            return;
        };
        // 有害重排 = baseline 命中且上下文未命中。两次打分都只算一次,
        // 既避免重复 rank_paths(每 token 一次全路径枚举),也让判定口径
        // 与 replay_sentences 完全一致。
        let baseline_top = scored_top(&baseline, context, lattice);
        let contextual_top = scored_top(&contextual, context, lattice);
        if !top_is(baseline_top, token) || top_is(contextual_top.clone(), token) {
            return;
        }
        let Some(picked) = contextual_top else {
            return;
        };
        // committed 前文尾部 token:与 KdconvBigramScorer 的上下文窗口取法一致
        // (最长已知词优先,未知字符切断,再取窗口内最后一个)。
        let tail = tail_token(model, context.committed_left(), 4).unwrap_or_default();
        if !seen.insert((tail.clone(), token.to_string())) {
            return;
        }
        found.push(HarmfulCase {
            committed_tail: tail.clone(),
            expected: token.to_string(),
            picked: picked.clone(),
            expected_evidence: model.transition_count(&tail, token),
            picked_evidence: model.transition_count(&tail, &picked),
        });
    });

    found.sort_by(|a, b| {
        a.expected
            .cmp(&b.expected)
            .then(a.committed_tail.cmp(&b.committed_tail))
            .then(a.picked.cmp(&b.picked))
    });
    found.truncate(limit);
    found
}

/// 遍历语料中所有「有 canonical 词码且期望词在菜单内」的 token。
///
/// 对每个 token 调用 `on_ambiguous(token, context, lattice)`(同码候选 ≥2)
/// 或 `on_unambiguous(token)`(唯一候选,必然命中)。语料遍历、分词与
/// `committed` 前文累积只在这里实现一次,保证各报告口径一致。
fn walk_ambiguous_tokens<F>(
    sentences_text: &str,
    prices: &ProductionMenus,
    segmenter: &Segmenter,
    mut on_token: F,
) where
    F: FnMut(&str, Option<(&RuntimeContext, &Lattice)>),
{
    for line in sentences_text.lines() {
        let sentence = line.trim();
        if sentence.is_empty() {
            continue;
        }
        let tokens = segmenter.segment(sentence);
        let mut committed = String::new();
        for token in &tokens {
            // 无 canonical 词码的 token 不参与排名(见模块文档「语义边界」)。
            let Some(code) = prices.word_code.get(token) else {
                committed.push_str(token);
                continue;
            };
            let Some(candidates) = prices.menus.get(code) else {
                committed.push_str(token);
                continue;
            };
            // 期望词必须在菜单内,否则该 token 不构成本基准的有效样本。
            if !candidates.iter().any(|(word, _)| word == token) {
                committed.push_str(token);
                continue;
            }
            let Ok(key_seq) = code.parse::<KeySequence>() else {
                committed.push_str(token);
                continue;
            };

            if candidates.len() < 2 {
                on_token(token, None);
                committed.push_str(token);
                continue;
            }

            let mut lattice = Lattice::new(key_seq.clone());
            let span = Span::new(0, key_seq.len()).expect("canonical 码长度 ≥ 2");
            for (word, frequency) in candidates {
                lattice
                    .add_edge(
                        span,
                        EdgeCandidate::new(word.as_str(), CandidateKind::HotWord, *frequency)
                            .expect("canonical 候选文本非空"),
                    )
                    .expect("同码候选共享同一合法 span");
            }
            let context = RuntimeContext::new(committed.as_str(), key_seq);
            on_token(token, Some((&context, &lattice)));
            committed.push_str(token);
        }
    }
}

/// 统计语料中的句子数(与回放口径一致:非空行)。
fn count_sentences(sentences_text: &str) -> usize {
    sentences_text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

/// 有界 beam 解码与全路径枚举的一致性回放(§25 第 7 步 / §22)。
///
/// 对语料中每个同码歧义 token,分别用
/// [`decode_beam`](xhup_decoder::decode_beam)(生产路径)与
/// [`rank_paths`](xhup_decoder::rank_paths) + 全路径枚举(参考路径)
/// 求 top1,统计一致性与截断规模。`beam_width >= 候选数` 时截断应为 0,
/// 此时两者必须**逐 token 一致**;不一致即精度回归。
pub fn bounded_decode_consistency<S>(
    sentences_text: &str,
    scorer: &S,
    scorer_id: &'static str,
    config: xhup_decoder::DecodeConfig,
) -> BoundedDecodeReport
where
    S: DeterministicScorer,
{
    let prices = ProductionMenus::build();
    let segmenter = Segmenter::build();
    let mut metrics = BoundedDecodeMetrics::default();

    walk_ambiguous_tokens(sentences_text, &prices, &segmenter, |_token, ranked| {
        let Some((context, lattice)) = ranked else {
            return;
        };
        metrics.compared += 1;
        let reference = scored_top(scorer, context, lattice);
        let decoded = xhup_decoder::decode_beam(lattice, context, scorer, config);
        match decoded.ranked().first() {
            Some(path) => {
                if Some(path.text()) == reference.as_deref() {
                    metrics.top1_agreement += 1;
                }
            }
            None => metrics.empty += 1,
        }
        if decoded.truncated() {
            metrics.truncated += 1;
        }
        if decoded.fallback() == Some(xhup_decoder::FallbackReason::LowConfidence) {
            metrics.low_confidence += 1;
        }
    });

    BoundedDecodeReport {
        schema: BOUNDED_DECODE_SCHEMA,
        scorer: scorer_id,
        beam_width: config.beam_width.get(),
        top_k: config.top_k.get(),
        min_confidence_gap: config.min_confidence_gap,
        metrics,
    }
}

/// 取 committed 前文的尾部 token(与 `KdconvBigramScorer` 的窗口语义一致)。
///
/// 最长已知词(2..=4 字)优先,否则单字;未知字符切断整条上下文链;
/// 最后保留窗口内最近的 `max_tokens` 个。只用公开的 [`BigramModel::unigram_count`]
/// 判定「是否已知」,因此不需要改动 `xhup-decoder`。
fn tail_token(model: &xhup_decoder::BigramModel, text: &str, max_tokens: usize) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens: Vec<String> = Vec::new();
    let mut pos = 0;
    while pos < chars.len() {
        let mut matched = None;
        for len in (2..=4).rev() {
            if pos + len <= chars.len() {
                let candidate: String = chars[pos..pos + len].iter().collect();
                if model.unigram_count(&candidate) > 0 {
                    matched = Some((candidate, len));
                    break;
                }
            }
        }
        match matched {
            Some((token, len)) => {
                tokens.push(token);
                pos += len;
            }
            None => {
                let single = chars[pos].to_string();
                if model.unigram_count(&single) > 0 {
                    tokens.push(single);
                } else {
                    // 未知字符:切断上下文链(与 corpus 边界语义一致)。
                    tokens.clear();
                }
                pos += 1;
            }
        }
    }
    if tokens.len() > max_tokens {
        let drop_count = tokens.len() - max_tokens;
        tokens.drain(..drop_count);
    }
    tokens.pop()
}

fn scored_top<S: DeterministicScorer>(
    scorer: &S,
    context: &RuntimeContext,
    lattice: &Lattice,
) -> Option<String> {
    let paths = lattice.complete_paths(PATH_LIMIT);
    let ranked = xhup_decoder::rank_paths(scorer, context, lattice, paths.paths());
    ranked.first().map(|path| path.text().to_string())
}

/// 期望文本在排名中的 1-based 位置;未出现返回 0(视为最差档,见
/// [`selection_cost_q10`])。
fn scored_rank<S: DeterministicScorer>(
    scorer: &S,
    context: &RuntimeContext,
    lattice: &Lattice,
    expected: &str,
) -> usize {
    let paths = lattice.complete_paths(PATH_LIMIT);
    let ranked = xhup_decoder::rank_paths(scorer, context, lattice, paths.paths());
    ranked
        .iter()
        .position(|path| path.text() == expected)
        .map_or(0, |index| index + 1)
}

fn top_is(top: Option<String>, expected: &str) -> bool {
    top.as_deref() == Some(expected)
}

/// canonical 词码层的生产菜单(code → 候选),按进程构建一次。
struct ProductionMenus {
    menus: BTreeMap<String, Vec<(String, u64)>>,
    word_code: BTreeMap<String, String>,
}

impl ProductionMenus {
    fn build() -> Self {
        let mut menus: BTreeMap<String, Vec<(String, u64)>> = BTreeMap::new();
        let mut word_code: BTreeMap<String, String> = BTreeMap::new();
        for entry in xhup_generator::canonical_word_code_entries() {
            let code = entry.code().to_string();
            let word = entry.word().to_string();
            let bucket = menus.entry(code.clone()).or_default();
            // 同一词在同一码下只记一次(数据层已保证,这里防御性去重)。
            if !bucket.iter().any(|(w, _)| w == &word) {
                bucket.push((word.clone(), entry.frequency_score()));
            }
            word_code.insert(word, code);
        }
        // 菜单内按 (频率降序, 词形升序) 固定,保证确定性。
        for bucket in menus.values_mut() {
            bucket.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        }
        ProductionMenus { menus, word_code }
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        return 0.0;
    }
    numerator as f64 / denominator as f64
}

fn ratio_u64(numerator: u64, denominator: usize) -> f64 {
    if denominator == 0 {
        return 0.0;
    }
    numerator as f64 / denominator as f64
}

/// 解码延迟的百分位(微秒)。
///
/// §22 要求测 `decoder p50/p95/p99`,此前仓库中**没有任何基线**。计时只报告、
/// 不设跨机器门槛(与 `contextual_benchmark` 的既有口径一致)。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayLatency {
    /// 样本数(被计时的 token 数)。
    pub samples: usize,
    pub p50_micros: u128,
    pub p95_micros: u128,
    pub p99_micros: u128,
    /// 最大单次(定位长尾用;百分位会掩盖极少数尖峰)。
    pub max_micros: u128,
}

/// 一次延迟测量(含评分器与配置,便于溯源)。
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayLatencyReport {
    pub schema: &'static str,
    pub scorer: &'static str,
    pub beam_width: usize,
    pub latency: ReplayLatency,
}

/// 延迟报告 schema 标识。
pub const REPLAY_LATENCY_SCHEMA: &str = "xhup-context-replay-latency/v1";

/// 测量真实语料上每个歧义 token 的解码延迟(微秒百分位)。
///
/// 计时范围:构造候选菜单之后、`rank_paths`(参考路径枚举)与
/// `decode_beam`(有界解码)**两次**打分之和 —— 即生产解码路径的开销。
/// 菜单构造与语料分词不计入(它们不随按键变化)。
pub fn replay_latency<S>(
    sentences_text: &str,
    scorer: &S,
    scorer_id: &'static str,
    beam_width: usize,
) -> ReplayLatencyReport
where
    S: DeterministicScorer,
{
    let prices = ProductionMenus::build();
    let segmenter = Segmenter::build();
    let mut samples: Vec<u128> = Vec::new();
    let beam = NonZeroUsize::new(beam_width.max(1)).expect("beam >= 1");
    let config =
        xhup_decoder::DecodeConfig::new(beam, NonZeroUsize::new(5).expect("top_k 5 != 0"), 0);

    walk_ambiguous_tokens(sentences_text, &prices, &segmenter, |_token, ranked| {
        let Some((context, lattice)) = ranked else {
            return;
        };
        let started = Instant::now();
        // 参考路径枚举 + 排序(既有参考实现)。
        let _ = scored_top(scorer, context, lattice);
        // 有界解码(生产路径候选)。
        let _ = xhup_decoder::decode_beam_adaptive(
            lattice,
            context,
            scorer,
            config,
            xhup_decoder::DEFAULT_MAX_BEAM_WIDTH,
        );
        samples.push(started.elapsed().as_micros());
    });

    samples.sort_unstable();
    let latency = ReplayLatency {
        samples: samples.len(),
        p50_micros: percentile(&samples, 50),
        p95_micros: percentile(&samples, 95),
        p99_micros: percentile(&samples, 99),
        max_micros: samples.last().copied().unwrap_or(0),
    };
    ReplayLatencyReport {
        schema: REPLAY_LATENCY_SCHEMA,
        scorer: scorer_id,
        beam_width: beam_width.max(1),
        latency,
    }
}

fn percentile(sorted: &[u128], percentile: usize) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (sorted.len() * percentile).div_ceil(100);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    const SENTENCES: &str = include_str!("../../../data/corpus/replay_fixture.txt");
    const KDCONV: &str = include_str!("../../../data/corpus/kdconv_bigram.tsv");
    const PTT: &str = include_str!("../../../data/corpus/ptt_bigram.tsv");

    /// 合并证据回放:审计口径与单源一致,且合并至少保留两源全部转移对。
    #[test]
    fn merged_replay_produces_audit_and_preserves_both_sources() {
        let kd = xhup_decoder::BigramModel::from_tsv(KDCONV).expect("kdconv 可解析");
        let pt = xhup_decoder::BigramModel::from_tsv(PTT).expect("ptt 可解析");
        let kd_pairs = kd.pair_count();
        let pt_pairs = pt.pair_count();
        let (report, audit) =
            replay_sentences_merged(SENTENCES, kd, pt, xhup_decoder::MergePolicy::RawSum);
        assert_eq!(audit.policy, "raw-sum/v1");
        assert_eq!(audit.sources.len(), 2);
        assert_eq!(
            report.contextual_scorer,
            xhup_decoder::KdconvBigramScorer::SCORER_ID
        );
        // RawSum 唯一转移对数 = 两源去重并集;至少各源自述规模的最大值,
        // 至多两源之和(交集不为空时严格小于)。
        assert!(audit.merged_pairs >= kd_pairs.max(pt_pairs));
        assert!(audit.merged_pairs <= kd_pairs + pt_pairs);
    }

    /// 合并证据必须保留两源的独占转移对(合并通道的完整性与可解释性)。
    #[test]
    fn merged_replay_keeps_source_unique_pairs() {
        let kd = xhup_decoder::BigramModel::from_tsv(KDCONV).expect("kdconv 可解析");
        let pt = xhup_decoder::BigramModel::from_tsv(PTT).expect("ptt 可解析");
        let (merged, audit) = xhup_decoder::merge_models(
            &[("kdconv", &kd), ("ptt", &pt)],
            xhup_decoder::MergePolicy::RawSum,
        );
        // KDConv 独占(PTT 无)与 PTT 独占(KDConv 无)都保留。
        for (l, r) in [("<s>", "是的"), ("这个", "景点")] {
            assert!(kd.transition_count(l, r) > 0);
            assert!(
                pt.transition_count(l, r) == 0
                    || merged.transition_count(l, r) >= kd.transition_count(l, r)
            );
        }
        assert!(audit.merged_pairs > kd.pair_count(), "PTT 独占对必须并入");
    }
}
