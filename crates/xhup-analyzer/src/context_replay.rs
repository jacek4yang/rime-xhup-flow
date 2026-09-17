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
            // 无同码歧义:两个 scorer 都必然命中。
            metrics.baseline_rank1 += 1;
            metrics.contextual_rank1 += 1;
            return;
        };
        metrics.ambiguous += 1;
        let baseline_ok = top_is(scored_top(&baseline, context, lattice), token);
        let contextual_ok = top_is(scored_top(contextual, context, lattice), token);
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

fn scored_top<S: DeterministicScorer>(
    scorer: &S,
    context: &RuntimeContext,
    lattice: &Lattice,
) -> Option<String> {
    let paths = lattice.complete_paths(PATH_LIMIT);
    let ranked = xhup_decoder::rank_paths(scorer, context, lattice, paths.paths());
    ranked.first().map(|path| path.text().to_string())
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
