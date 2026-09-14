//! 从结构化多源候选批量构建融合 lattice 的入口。
//!
//! 里程碑一「生产级 Joint Key/Text Lattice 构造与多源候选融合」:
//! [`LatticeBuilder`] 接收各来源(静态层、词库层、单字原语、学习层、
//! 组句层等)的 `(span, 文本, 频率)` 事实,经 [`Lattice::add_edge`] 的
//! 确定性融合契约收拢为单一边——同一 `(span, text)` 恰好一条边、
//! evidence 为来源并集。构建过程记录融合统计,供审计与 Trainer 解释。

use std::num::NonZeroUsize;

use crate::{
    CandidateEvidence, CandidateKind, EdgeCandidate, Lattice, LatticeError, PathSet, Span,
};
use xhup_core::KeySequence;

/// 单来源的一条候选事实。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCandidate {
    span: (usize, usize),
    text: String,
    kind: CandidateKind,
    frequency: u64,
}

impl SourceCandidate {
    pub fn new(
        span: (usize, usize),
        text: impl Into<String>,
        kind: CandidateKind,
        frequency: u64,
    ) -> Self {
        Self {
            span,
            text: text.into(),
            kind,
            frequency,
        }
    }

    pub fn span(&self) -> (usize, usize) {
        self.span
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn kind(&self) -> CandidateKind {
        self.kind
    }

    pub fn frequency(&self) -> u64 {
        self.frequency
    }
}

/// 多源候选 → 融合 lattice 的构建统计。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BuildStats {
    /// 输入事实总数(全部来源)。
    pub facts: usize,
    /// 融合后 lattice 的边数(唯一 `(span, text)` 数)。
    pub edges: usize,
    /// 因与既有 `(span, text)` 相同而被融合掉的输入条数。
    pub fused_facts: usize,
}

impl BuildStats {
    /// 发生融合的来源对数 = facts - edges(每融合一次少一条边)。
    pub fn fused_pairs(&self) -> usize {
        self.facts - self.edges
    }
}

/// 一次构建的产物:融合 lattice、完整路径集合(有界)与统计。
#[derive(Clone)]
pub struct BuiltLattice {
    lattice: Lattice,
    paths: PathSet,
    stats: BuildStats,
}

impl std::fmt::Debug for BuiltLattice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuiltLattice")
            .field("stats", &self.stats)
            .field("truncated", &self.paths.truncated())
            .finish_non_exhaustive()
    }
}

impl BuiltLattice {
    pub fn lattice(&self) -> &Lattice {
        &self.lattice
    }

    pub fn paths(&self) -> &PathSet {
        &self.paths
    }

    pub fn stats(&self) -> &BuildStats {
        &self.stats
    }
}

/// 按 (span 起点, span 长度, 文本) 分组批量构建。
#[derive(Clone, Debug, Default)]
pub struct LatticeBuilder {
    facts: Vec<SourceCandidate>,
}

impl LatticeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, fact: SourceCandidate) {
        self.facts.push(fact);
    }

    /// 按来源顺序批量构建。facts 为空时返回错误(空 lattice 无意义)。
    ///
    /// `path_limit` 传给 [`Lattice::complete_paths`] 的显式上限。
    pub fn build(
        self,
        input: &str,
        path_limit: NonZeroUsize,
    ) -> Result<BuiltLattice, LatticeError> {
        if self.facts.is_empty() {
            return Err(LatticeError::EmptyCandidateText);
        }
        let composition: KeySequence = input
            .parse()
            .map_err(|_| LatticeError::InvalidSpan { start: 0, end: 0 })?;
        let mut lattice = Lattice::new(composition);
        let mut stats = BuildStats {
            facts: self.facts.len(),
            edges: 0,
            fused_facts: 0,
        };
        for fact in &self.facts {
            let span = Span::new(fact.span.0, fact.span.1)?;
            let candidate = EdgeCandidate::new(fact.text.as_str(), fact.kind, fact.frequency)?;
            let before = lattice.edges().len();
            lattice.add_edge(span, candidate)?;
            if lattice.edges().len() == before {
                stats.fused_facts += 1;
            }
        }
        stats.edges = lattice.edges().len();
        let paths = lattice.complete_paths(path_limit);
        Ok(BuiltLattice {
            lattice,
            paths,
            stats,
        })
    }

    /// 便捷构造:直接以 evidence 形式提交(允许单条事实自带多来源)。
    pub fn push_multi(
        &mut self,
        span: (usize, usize),
        text: String,
        evidence: &[CandidateEvidence],
    ) {
        for e in evidence {
            self.facts.push(SourceCandidate::new(
                span,
                text.clone(),
                e.kind(),
                e.frequency(),
            ));
        }
    }
}
