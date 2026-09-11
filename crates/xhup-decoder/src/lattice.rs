//! 同一原始按键上并存多种解释的有向无环 lattice。

use std::error::Error;
use std::fmt;
use std::num::NonZeroUsize;

use xhup_core::KeySequence;

/// 半开按键区间 `[start, end)`；偏移以 ASCII 按键数计。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Span {
    start: usize,
    end: usize,
}

impl Span {
    /// 构造非空、正向 span。是否越过 composition 末尾由 [`Lattice`] 校验。
    pub fn new(start: usize, end: usize) -> Result<Self, LatticeError> {
        if start >= end {
            return Err(LatticeError::InvalidSpan { start, end });
        }
        Ok(Self { start, end })
    }

    pub fn start(self) -> usize {
        self.start
    }

    pub fn end(self) -> usize {
        self.end
    }

    pub fn len(self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(self) -> bool {
        false
    }
}

/// lattice edge 的来源类别。类别影响解释与后续安全策略，但不决定可达性。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum CandidateKind {
    HotWord,
    ExtendedWord,
    Character,
    AttestedAlias,
    UserLearned,
    OovComposition,
}

/// 一条 span 解释携带的最小候选事实。
#[derive(Clone, Eq, PartialEq)]
pub struct EdgeCandidate {
    text: Box<str>,
    kind: CandidateKind,
    frequency: u64,
}

impl EdgeCandidate {
    /// frequency 为零表示“缺少频率证据”，不表示文本无效或不可达。
    pub fn new(
        text: impl Into<Box<str>>,
        kind: CandidateKind,
        frequency: u64,
    ) -> Result<Self, LatticeError> {
        let text = text.into();
        if text.is_empty() {
            return Err(LatticeError::EmptyCandidateText);
        }
        Ok(Self {
            text,
            kind,
            frequency,
        })
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

/// lattice 内稳定的 edge 标识；仅在所属 lattice 内有意义。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct EdgeId(usize);

impl EdgeId {
    pub fn index(self) -> usize {
        self.0
    }
}

/// 一条从 span 起点到终点的候选边。
#[derive(Clone, Eq, PartialEq)]
pub struct LatticeEdge {
    id: EdgeId,
    span: Span,
    candidate: EdgeCandidate,
}

impl LatticeEdge {
    pub fn id(&self) -> EdgeId {
        self.id
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn candidate(&self) -> &EdgeCandidate {
        &self.candidate
    }
}

/// 一条覆盖完整 composition 的分段路径。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LatticePath {
    edge_ids: Box<[EdgeId]>,
}

impl LatticePath {
    pub fn edge_ids(&self) -> &[EdgeId] {
        &self.edge_ids
    }

    pub fn len(&self) -> usize {
        self.edge_ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.edge_ids.is_empty()
    }
}

/// 显式上限下枚举到的完整路径；`truncated` 表示仍有未物化路径。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathSet {
    paths: Vec<LatticePath>,
    truncated: bool,
}

impl PathSet {
    pub fn paths(&self) -> &[LatticePath] {
        &self.paths
    }

    pub fn truncated(&self) -> bool {
        self.truncated
    }
}

/// 单个 composition 的候选 DAG。所有 edge 严格前进，因此不会形成环。
#[derive(Clone)]
pub struct Lattice {
    input: KeySequence,
    edges: Vec<LatticeEdge>,
    outgoing: Vec<Vec<EdgeId>>,
}

impl Lattice {
    pub fn new(input: KeySequence) -> Self {
        let outgoing = vec![Vec::new(); input.len() + 1];
        Self {
            input,
            edges: Vec::new(),
            outgoing,
        }
    }

    pub fn input(&self) -> &KeySequence {
        &self.input
    }

    pub fn edges(&self) -> &[LatticeEdge] {
        &self.edges
    }

    pub fn edge(&self, id: EdgeId) -> Option<&LatticeEdge> {
        self.edges.get(id.0)
    }

    pub fn outgoing(&self, position: usize) -> Option<&[EdgeId]> {
        self.outgoing.get(position).map(Vec::as_slice)
    }

    /// 加入一条候选边。插入顺序成为稳定 EdgeId；不做候选去重，以便后续证据层
    /// 明确表示同一文本的不同来源。
    pub fn add_edge(
        &mut self,
        span: Span,
        candidate: EdgeCandidate,
    ) -> Result<EdgeId, LatticeError> {
        if span.end > self.input.len() {
            return Err(LatticeError::SpanOutOfBounds {
                start: span.start,
                end: span.end,
                input_len: self.input.len(),
            });
        }
        let id = EdgeId(self.edges.len());
        self.edges.push(LatticeEdge {
            id,
            span,
            candidate,
        });
        self.outgoing[span.start].push(id);
        Ok(id)
    }

    /// 以显式上限枚举完整路径。该方法只是里程碑一的可验证参考实现；生产
    /// 解码器将使用有界 Beam/Viterbi，不会先物化所有路径。
    pub fn complete_paths(&self, limit: NonZeroUsize) -> PathSet {
        let mut paths = Vec::new();
        let mut current = Vec::new();
        // 至多多收集一条路径来精确判断 truncated；不会为了判断截断而遍历
        // 剩余完整 DAG。usize::MAX 不可能再为 Vec 增加一条，按未截断处理。
        let probe_limit = limit.get().checked_add(1).unwrap_or(limit.get());
        self.visit_paths(0, probe_limit, &mut current, &mut paths);
        let truncated = paths.len() > limit.get();
        paths.truncate(limit.get());
        PathSet { paths, truncated }
    }

    fn visit_paths(
        &self,
        position: usize,
        limit: usize,
        current: &mut Vec<EdgeId>,
        paths: &mut Vec<LatticePath>,
    ) -> bool {
        if position == self.input.len() {
            paths.push(LatticePath {
                edge_ids: current.clone().into_boxed_slice(),
            });
            return paths.len() >= limit;
        }
        for &edge_id in &self.outgoing[position] {
            if paths.len() >= limit {
                return true;
            }
            let edge = &self.edges[edge_id.0];
            current.push(edge_id);
            let limit_reached = self.visit_paths(edge.span.end, limit, current, paths);
            current.pop();
            if limit_reached {
                return true;
            }
        }
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LatticeError {
    InvalidSpan {
        start: usize,
        end: usize,
    },
    SpanOutOfBounds {
        start: usize,
        end: usize,
        input_len: usize,
    },
    EmptyCandidateText,
}

impl fmt::Display for LatticeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSpan { start, end } => {
                write!(f, "lattice span 必须非空且前进: {start}..{end}")
            }
            Self::SpanOutOfBounds {
                start,
                end,
                input_len,
            } => write!(
                f,
                "lattice span 越过输入末尾: {start}..{end},输入长度 {input_len}"
            ),
            Self::EmptyCandidateText => write!(f, "lattice 候选文本不能为空"),
        }
    }
}

impl Error for LatticeError {}
