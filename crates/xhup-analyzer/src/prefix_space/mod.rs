//! Prefix-Space Compiler v3: 前缀闭合变长码空间编译器与全局候选优化。
//!
//! 核心原则:
//! - 前缀闭合码空间 Trie 组织结构
//! - 有效前缀不等于提交边界 (`valid prefix != commit boundary`)
//! - 码位是有序候选槽位 `(code, rank)`, 不由单一词语独占
//! - 目标可在多个前缀长度以不同 rank 阶梯可达
//! - 全局确定性多目标优化(词频、击键、候选位、碰撞、拥塞、外部性、歧义、规则偏离、迁移成本)
//! - 决策透明可解释, 支持 CLI 理由卡与详细 TSV 导出
//! - 提供与 Optimizer v2 的确定性对比基准

pub mod benchmark;
pub mod cost;
pub mod explain;
pub mod slot;
pub mod solver;
pub mod trie;

pub use benchmark::{BenchmarkMetrics, PrefixSpaceBenchmarkReport, evaluate_v3_metrics};
pub use cost::{PrefixCostModel, PrefixUtilityBreakdown, evaluate_prefix_placement};
pub use explain::PrefixPlacementExplanation;
pub use slot::{CandidateSlotKey, CandidateSlotState, SlotCandidate, SlotPlacementSource};
pub use solver::{
    MAX_SLOTS_PER_NODE, PrefixSpaceCompiledModel, PrefixSpaceStats, PrefixTarget, SolverOptions,
    solve_prefix_space,
};
pub use trie::{PrefixTrie, TrieNode};
