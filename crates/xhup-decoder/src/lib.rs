//! XHUP Flow 上下文联合解码领域基础。
//!
//! 本 crate 只定义与平台无关的运行时上下文、按键 span lattice、完整路径与
//! 确定性评分契约。它不读取词典、不访问网络、不持久化用户文本，也不接入
//! librime；候选检索、Lua 编排与有界 Beam/Viterbi 解码由后续里程碑实现。
#![forbid(unsafe_code)]

mod context;
mod lattice;
mod scoring;

pub use context::RuntimeContext;
pub use lattice::{
    CandidateKind, EdgeCandidate, EdgeId, Lattice, LatticeEdge, LatticeError, LatticePath, PathSet,
    Span,
};
pub use scoring::{
    BaselineScoreBreakdown, BaselineScorer, DeterministicScorer, Score, ScoredPath, rank_paths,
};
