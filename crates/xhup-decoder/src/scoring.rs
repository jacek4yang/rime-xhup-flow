//! 固定点确定性评分接口与词频+分段 baseline。

use crate::{Lattice, LatticePath, RuntimeContext};

/// 评分统一使用固定点整数；越大越好，避免跨平台浮点决胜漂移。
pub type Score = i64;

/// scorer 的最小接口。实现必须在相同 context、lattice 与 path 下返回相同结果，
/// 且不得修改外部状态。
pub trait DeterministicScorer {
    type Breakdown;

    fn score(
        &self,
        context: &RuntimeContext,
        lattice: &Lattice,
        path: &LatticePath,
    ) -> (Score, Self::Breakdown);
}

/// baseline 的可解释分数组成。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BaselineScoreBreakdown {
    pub lexical_frequency: Score,
    pub segmentation_penalty: Score,
    pub total: Score,
}

/// 里程碑一 baseline：整数 log2 词频奖励减去每段固定惩罚。
///
/// 它刻意不使用 left context；同一输入在不同上下文下会得到相同排序。该可测缺口
/// 是后续 n-gram/context scorer 的对照基线。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BaselineScorer {
    segment_penalty: Score,
}

impl BaselineScorer {
    pub const SCORER_ID: &'static str = "word-frequency-segmentation/v1";

    pub const fn new(segment_penalty: Score) -> Self {
        Self { segment_penalty }
    }

    pub const fn segment_penalty(self) -> Score {
        self.segment_penalty
    }

    /// Q10 固定点 log2(frequency + 1)。零频率是“无证据”而非不可达。
    fn frequency_reward(frequency: u64) -> Score {
        let value = frequency.saturating_add(1);
        let exponent = value.ilog2();
        let base = 1_u64 << exponent;
        let fractional = (((value - base) as u128) * 1024 / base as u128) as Score;
        Score::from(exponent) * 1024 + fractional
    }
}

impl Default for BaselineScorer {
    fn default() -> Self {
        Self::new(4096)
    }
}

impl DeterministicScorer for BaselineScorer {
    type Breakdown = BaselineScoreBreakdown;

    fn score(
        &self,
        _context: &RuntimeContext,
        lattice: &Lattice,
        path: &LatticePath,
    ) -> (Score, Self::Breakdown) {
        let lexical_frequency = path.edge_ids().iter().fold(0_i64, |sum, &id| {
            let edge = lattice.edge(id).expect("path edge 必须属于 lattice");
            sum.saturating_add(Self::frequency_reward(edge.candidate().frequency()))
        });
        let segmentation_penalty = self
            .segment_penalty
            .saturating_mul(path.len().try_into().unwrap_or(i64::MAX));
        let total = lexical_frequency.saturating_sub(segmentation_penalty);
        (
            total,
            BaselineScoreBreakdown {
                lexical_frequency,
                segmentation_penalty,
                total,
            },
        )
    }
}

/// 已评分完整路径；包含解释所需的 surface text 与分段。
#[derive(Clone, Eq, PartialEq)]
pub struct ScoredPath<B> {
    path: LatticePath,
    text: String,
    segments: Vec<String>,
    score: Score,
    breakdown: B,
}

impl<B> ScoredPath<B> {
    pub fn path(&self) -> &LatticePath {
        &self.path
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    pub fn score(&self) -> Score {
        self.score
    }

    pub fn breakdown(&self) -> &B {
        &self.breakdown
    }
}

/// 对已被显式上限约束的路径集评分并确定性排序。
///
/// 决胜顺序：总分降序 → 段数升序 → surface text 升序 → 分段文本升序 →
/// EdgeId 序列升序。不会依赖 HashMap 顺序或浮点比较。
pub fn rank_paths<S: DeterministicScorer>(
    scorer: &S,
    context: &RuntimeContext,
    lattice: &Lattice,
    paths: &[LatticePath],
) -> Vec<ScoredPath<S::Breakdown>> {
    let mut scored = Vec::with_capacity(paths.len());
    for path in paths {
        let mut text = String::new();
        let mut segments = Vec::with_capacity(path.len());
        for &id in path.edge_ids() {
            let segment = lattice
                .edge(id)
                .expect("path edge 必须属于 lattice")
                .candidate()
                .text();
            text.push_str(segment);
            segments.push(segment.to_string());
        }
        let (score, breakdown) = scorer.score(context, lattice, path);
        scored.push(ScoredPath {
            path: path.clone(),
            text,
            segments,
            score,
            breakdown,
        });
    }
    scored.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(a.path.len().cmp(&b.path.len()))
            .then(a.text.cmp(&b.text))
            .then(a.segments.cmp(&b.segments))
            .then(a.path.edge_ids().cmp(b.path.edge_ids()))
    });
    scored
}
