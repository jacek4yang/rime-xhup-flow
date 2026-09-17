//! 有界 Beam/Viterbi 联合解码基础。
//!
//! 在已有 [`Lattice`] 上从左到右展开假设，每一位置最多保留 `beam_width` 条；
//! 完整路径再用调用方提供的 [`DeterministicScorer`] 重打分。生产解码不再
//! 依赖 [`Lattice::complete_paths`] 物化全部路径；后者仍是可验证参考枚举器。

use std::cmp::Ordering;
use std::fmt;
use std::num::NonZeroUsize;

use crate::scoring::{BaselineScorer, DeterministicScorer, Score, log2_q10, rank_paths};
use crate::{EdgeId, Lattice, LatticePath, RuntimeContext, ScoredPath};

/// beam 搜索与 Top-K 截取的显式上限。
///
/// `min_confidence_gap == 0` 关闭低置信回退；大于 0 时，若 top1 与 top2
/// 分差小于该值则报告 [`FallbackReason::LowConfidence`]。回退只标记、不改排序。
///
/// 本结构描述**单次**有界搜索的固定宽度。需要「够宽就等价于穷举」的语义时
/// 用 [`decode_beam_adaptive`]，它按实际截断情况在 `[beam_width, max_width]`
/// 内自适应扩宽。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeConfig {
    pub beam_width: NonZeroUsize,
    pub top_k: NonZeroUsize,
    pub min_confidence_gap: Score,
}

impl DecodeConfig {
    pub const fn new(
        beam_width: NonZeroUsize,
        top_k: NonZeroUsize,
        min_confidence_gap: Score,
    ) -> Self {
        Self {
            beam_width,
            top_k,
            min_confidence_gap,
        }
    }
}

impl Default for DecodeConfig {
    fn default() -> Self {
        Self {
            beam_width: NonZeroUsize::new(8).expect("默认 beam_width 8 ≠ 0"),
            top_k: NonZeroUsize::new(5).expect("默认 top_k 5 ≠ 0"),
            min_confidence_gap: 0,
        }
    }
}

/// 自适应扩宽的安全上限。
///
/// 真实数据实测（`context-replay-bench --bounded` 扫描 2701 个同码歧义 token）：
/// 菜单扇出未超过 32，且 `beam_width >= 32` 时与穷举排序逐 token 一致。默认
/// 上限取该实测值，既能覆盖真实菜单，又把最坏情况的搜索规模钉死。
pub const DEFAULT_MAX_BEAM_WIDTH: NonZeroUsize = NonZeroUsize::new(32).expect("32 != 0");

/// 自适应扩宽的结果：最终宽度与是否仍被截断。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdaptiveOutcome {
    /// 实际使用的 beam 宽度（`beam_width <= used <= max_width`）。
    pub used_width: usize,
    /// 达到 `max_width` 后仍有截断。
    pub truncated_at_max: bool,
}

/// 低置信或 beam 截断时的回退原因。本基础实现只报告、不改排序。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FallbackReason {
    /// 某一位置的候选数超过 `beam_width`，部分假设被丢弃。
    BeamTruncated,
    /// 未截断，但 top1 与 top2 分差小于配置的 `min_confidence_gap`。
    LowConfidence,
}

/// 有界解码产物。`ranked` 已按 [`rank_paths`] 决胜序排列并截取 `top_k`。
#[derive(Clone, Eq, PartialEq)]
pub struct DecodeResult<B> {
    ranked: Vec<ScoredPath<B>>,
    truncated: bool,
    fallback: Option<FallbackReason>,
}

impl<B> DecodeResult<B> {
    pub fn ranked(&self) -> &[ScoredPath<B>] {
        &self.ranked
    }

    pub fn truncated(&self) -> bool {
        self.truncated
    }

    pub fn fallback(&self) -> Option<FallbackReason> {
        self.fallback
    }
}

impl<B> fmt::Debug for DecodeResult<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecodeResult")
            .field(
                "ranked_scores",
                &self
                    .ranked
                    .iter()
                    .map(ScoredPath::score)
                    .collect::<Vec<_>>(),
            )
            .field("truncated", &self.truncated)
            .field("fallback", &self.fallback)
            .finish()
    }
}

/// 前缀假设：增量 baseline 边分 + 已覆盖文本，供位置内确定性剪枝。
struct Hypothesis {
    score: Score,
    text: String,
    segments: Vec<String>,
    edge_ids: Vec<EdgeId>,
}

impl Hypothesis {
    fn root() -> Self {
        Self {
            score: 0,
            text: String::new(),
            segments: Vec::new(),
            edge_ids: Vec::new(),
        }
    }

    fn extend(&self, edge_id: EdgeId, segment: &str, edge_score: Score) -> Self {
        let mut text = String::with_capacity(self.text.len() + segment.len());
        text.push_str(&self.text);
        text.push_str(segment);
        let mut segments = Vec::with_capacity(self.segments.len() + 1);
        segments.extend_from_slice(&self.segments);
        segments.push(segment.to_string());
        let mut edge_ids = Vec::with_capacity(self.edge_ids.len() + 1);
        edge_ids.extend_from_slice(&self.edge_ids);
        edge_ids.push(edge_id);
        Self {
            score: self.score.saturating_add(edge_score),
            text,
            segments,
            edge_ids,
        }
    }

    fn into_path(self) -> LatticePath {
        LatticePath::from_edge_ids(self.edge_ids)
    }
}

/// 与 [`rank_paths`] 相同的决胜序：分数降序 → 段数升序 → 文本升序 →
/// 分段升序 → EdgeId 序列升序。
fn cmp_hypothesis(a: &Hypothesis, b: &Hypothesis) -> Ordering {
    b.score
        .cmp(&a.score)
        .then(a.edge_ids.len().cmp(&b.edge_ids.len()))
        .then(a.text.cmp(&b.text))
        .then(a.segments.cmp(&b.segments))
        .then(a.edge_ids.cmp(&b.edge_ids))
}

fn retain_beam(beam: &mut Vec<Hypothesis>, beam_width: usize) -> bool {
    let overflow = beam.len() > beam_width;
    beam.sort_by(cmp_hypothesis);
    beam.truncate(beam_width);
    overflow
}

fn empty_result<B>() -> DecodeResult<B> {
    DecodeResult {
        ranked: Vec::new(),
        truncated: false,
        fallback: None,
    }
}

/// 在 `lattice` 上做确定性有界 beam 搜索，再对存活完整路径用 `scorer` 重打分。
///
/// beam 展开使用与 [`BaselineScorer`] 相同的可加边分（`log2(freq+1)` Q10
/// 减去每段惩罚），以便在不调用完整路径 scorer 的前提下剪枝。到达输入末尾
/// 的假设经 [`rank_paths`] 排序后截取 `top_k`。
///
/// 任一位置候选数超过 `beam_width` 时 `truncated` 为真。无完整路径时返回
/// 空结果且 `truncated == false`，不会 panic。
///
/// 若需要「足够宽时与穷举排序等价」的保证，用 [`decode_beam_adaptive`]。
pub fn decode_beam<S: DeterministicScorer>(
    lattice: &Lattice,
    context: &RuntimeContext,
    scorer: &S,
    config: DecodeConfig,
) -> DecodeResult<S::Breakdown> {
    decode_beam_with_width(lattice, context, scorer, config, config.beam_width.get())
}

/// 在 `[beam_width, max_width]` 内自适应扩宽的有界解码。
///
/// 先用 `beam_width` 搜索；若发生截断则倍增宽度重试，直到不截断或达到
/// `max_width`。语义保证：
///
/// - 返回宽度为 `beam_width` 时，结果与 [`decode_beam`] 完全一致；
/// - `truncated_at_max == false` 时，beam 宽度已覆盖真实扇出，因此结果与
///   「物化全部完整路径再 [`rank_paths`]」**逐路径一致**；
/// - 仍截断时诚实报告 `truncated_at_max`，绝不假装等价。
///
/// 搜索是确定性的（同一输入必得同一结果），扩宽只是重跑同一算法的更宽配置。
pub fn decode_beam_adaptive<S: DeterministicScorer>(
    lattice: &Lattice,
    context: &RuntimeContext,
    scorer: &S,
    config: DecodeConfig,
    max_width: NonZeroUsize,
) -> (DecodeResult<S::Breakdown>, AdaptiveOutcome) {
    let floor = config.beam_width.get();
    let ceiling = max_width.get().max(floor);
    let mut width = floor;
    loop {
        let result = decode_beam_with_width(lattice, context, scorer, config, width);
        if !result.truncated() || width >= ceiling {
            let outcome = AdaptiveOutcome {
                used_width: width,
                truncated_at_max: result.truncated(),
            };
            return (result, outcome);
        }
        // 倍增并夹到上限；至少推进 1 以避免死循环。
        width = (width.saturating_mul(2)).min(ceiling);
    }
}

/// 以显式宽度执行一次 beam 搜索（`decode_beam` / `decode_beam_adaptive` 的共用核心）。
fn decode_beam_with_width<S: DeterministicScorer>(
    lattice: &Lattice,
    context: &RuntimeContext,
    scorer: &S,
    config: DecodeConfig,
    beam_width: usize,
) -> DecodeResult<S::Breakdown> {
    let n = lattice.input().len();
    let segment_penalty = BaselineScorer::default().segment_penalty();
    let mut beams: Vec<Vec<Hypothesis>> = (0..=n).map(|_| Vec::new()).collect();
    beams[0].push(Hypothesis::root());
    let mut truncated = false;

    for pos in 0..=n {
        if retain_beam(&mut beams[pos], beam_width) {
            truncated = true;
        }
        if pos == n {
            break;
        }
        let outgoing = lattice.outgoing(pos).unwrap_or(&[]);
        let current = std::mem::take(&mut beams[pos]);
        for hyp in &current {
            for &edge_id in outgoing {
                let edge = lattice
                    .edge(edge_id)
                    .expect("outgoing edge 必须属于 lattice");
                let segment = edge.candidate().text();
                let edge_score =
                    log2_q10(edge.candidate().frequency()).saturating_sub(segment_penalty);
                beams[edge.span().end()].push(hyp.extend(edge_id, segment, edge_score));
            }
        }
    }

    if beams[n].is_empty() {
        return empty_result();
    }

    let paths: Vec<LatticePath> = std::mem::take(&mut beams[n])
        .into_iter()
        .map(Hypothesis::into_path)
        .collect();
    let mut ranked = rank_paths(scorer, context, lattice, &paths);
    ranked.truncate(config.top_k.get());

    let fallback = if truncated {
        Some(FallbackReason::BeamTruncated)
    } else if config.min_confidence_gap > 0
        && ranked.len() >= 2
        && ranked[0].score().saturating_sub(ranked[1].score()) < config.min_confidence_gap
    {
        Some(FallbackReason::LowConfidence)
    } else {
        None
    };

    DecodeResult {
        ranked,
        truncated,
        fallback,
    }
}
