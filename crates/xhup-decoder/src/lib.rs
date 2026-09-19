//! XHUP Flow 上下文联合解码领域基础。
//!
//! 本 crate 只定义与平台无关的运行时上下文、按键 span lattice、完整路径、
//! 确定性评分契约与有界 Beam/Viterbi 解码。它不读取词典、不访问网络、不持久化
//! 用户文本，也不接入 librime；候选检索与 Lua 编排由后续里程碑实现。
//!
//! [`Lattice::complete_paths`] 是显式上限的参考枚举器，只应用于小 fixture。
//! 生产解码使用 [`decode_beam`]：按位置保留有界假设，不先物化全部路径。
#![forbid(unsafe_code)]

mod bigram;
mod builder;
mod context;
mod decode;
mod evidence;
mod lattice;
mod multi_source;
mod scoring;
mod user_boost;

pub use bigram::{BOS, BigramModel, EOS, KdconvBigramBreakdown, KdconvBigramScorer};
pub use builder::{BuildStats, BuiltLattice, LatticeBuilder, SourceCandidate};
pub use context::RuntimeContext;
pub use decode::{
    AdaptiveOutcome, DEFAULT_MAX_BEAM_WIDTH, DecodeConfig, DecodeResult, FallbackReason,
    decode_beam, decode_beam_adaptive,
};
pub use evidence::{
    CandidateSource, FusedCandidate, FusedEdge, FusedEdgeId, LatticeBounds, LatticeBuildStats,
    LatticeFusionBuilder, SourceEvidence,
};
pub use lattice::{
    CandidateEvidence, CandidateKind, EdgeCandidate, EdgeId, Lattice, LatticeEdge, LatticeError,
    LatticePath, PathSet, Span,
};
pub use multi_source::{MergeAudit, MergePolicy, NORMALIZE_SCALE, merge_models};
pub use scoring::{
    BaselineScoreBreakdown, BaselineScorer, DeterministicScorer, Score, ScoredPath, rank_paths,
};
pub use user_boost::{USER_BOOST_SCORER_ID, UserBoostBreakdown, UserBoostScorer};
