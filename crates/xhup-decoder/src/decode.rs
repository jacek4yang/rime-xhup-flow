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
pub fn decode_beam<S: DeterministicScorer>(
    lattice: &Lattice,
    context: &RuntimeContext,
    scorer: &S,
    config: DecodeConfig,
) -> DecodeResult<S::Breakdown> {
    let n = lattice.input().len();
    let beam_width = config.beam_width.get();
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
