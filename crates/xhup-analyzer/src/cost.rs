//! 成本模型:全部可调参数集中于 [`CostModel`],不散落 magic number。
//!
//! 成本是**无量纲的优化目标**,不声称对应真实毫秒耗时:
//!
//! ```text
//! effective_cost = typed_keys + selection(rank) + ambiguity(fanout) [+ mode_complexity]
//! ```
//!
//! baseline 与 shortcut 都使用各自状态下的真实/projected rank 与 fanout。

use crate::candidates::ShortcutMode;

/// 一次输入动作的成本拆解,供报告解释每个分数的来源。
#[derive(Clone, Copy, Debug, Default)]
pub struct CostBreakdown {
    /// 实际按键数。
    pub typed_keys: f64,
    /// 名次选择成本。
    pub selection: f64,
    /// 候选歧义成本。
    pub ambiguity: f64,
    /// 投影模式复杂度成本(默认 0)。
    pub mode_complexity: f64,
}

impl CostBreakdown {
    /// 合计有效成本。
    pub fn total(&self) -> f64 {
        self.typed_keys + self.selection + self.ambiguity + self.mode_complexity
    }
}

/// 集中式可调成本模型。
#[derive(Clone, Copy, Debug)]
pub struct CostModel {
    /// rank 1 的选择成本(首候选直接空格上屏)。
    pub selection_rank1: f64,
    /// rank 2..=9 的选择成本(数字键选重)。
    pub selection_rank2_9: f64,
    /// rank ≥ 10 的选择成本(翻页)。
    pub selection_rank10_plus: f64,
    /// 歧义成本系数:`ambiguity = coeff × log2(max(fanout, 1))`(fanout 1 → 0)。
    pub ambiguity_coeff: f64,
    /// 既有候选扰动成本系数(仅 OPTIMIZED 非零效果;FIXED_FIRST 扰动恒为 0)。
    pub disruption_coeff: f64,
    /// 每次 F/I 模式切换的复杂度惩罚;主模型默认 0(无实证依据),sweep 中探索。
    pub mode_complexity_per_transition: f64,
}

impl CostModel {
    /// balanced operating point 的默认参数。
    pub fn balanced() -> Self {
        CostModel {
            selection_rank1: 0.0,
            selection_rank2_9: 1.0,
            selection_rank10_plus: 2.0,
            ambiguity_coeff: 0.5,
            disruption_coeff: 1.0,
            mode_complexity_per_transition: 0.0,
        }
    }

    /// 名次选择成本。
    pub fn selection_cost(&self, rank: u32) -> f64 {
        match rank {
            0 => 0.0,
            1 => self.selection_rank1,
            2..=9 => self.selection_rank2_9,
            _ => self.selection_rank10_plus,
        }
    }

    /// 候选歧义成本:fanout 1 → 0,2 → coeff,4 → 2×coeff,…(无量纲)。
    pub fn ambiguity_cost(&self, fanout: usize) -> f64 {
        self.ambiguity_coeff * (fanout.max(1) as f64).log2()
    }

    /// 投影模式复杂度成本:F/I 切换次数 × 单次惩罚(默认 0)。
    pub fn mode_complexity_cost(&self, mode: &ShortcutMode) -> f64 {
        self.mode_complexity_per_transition * mode.transitions() as f64
    }

    /// 完整码 baseline 的有效成本(无模式项)。
    pub fn baseline_cost(&self, full_length: usize, rank: u32, fanout: usize) -> CostBreakdown {
        CostBreakdown {
            typed_keys: full_length as f64,
            selection: self.selection_cost(rank),
            ambiguity: self.ambiguity_cost(fanout),
            mode_complexity: 0.0,
        }
    }

    /// shortcut 的有效成本(使用插入后的 projected rank/fanout)。
    pub fn shortcut_cost(
        &self,
        shortcut_length: usize,
        projected_rank: u32,
        projected_fanout: usize,
        mode: &ShortcutMode,
    ) -> CostBreakdown {
        CostBreakdown {
            typed_keys: shortcut_length as f64,
            selection: self.selection_cost(projected_rank),
            ambiguity: self.ambiguity_cost(projected_fanout),
            mode_complexity: self.mode_complexity_cost(mode),
        }
    }
}
