//! Prefix-Space 全局目标函数与成本模型。
//!
//! 目标函数多维度权衡:
//! 1. 词汇频率与会话频率 (lexical & conversational frequency)
//! 2. 预期击键收益 (expected keystrokes benefit)
//! 3. 候选位选择成本 (candidate rank cost)
//! 4. 码位碰撞竞争 (code collisions & occupant mass)
//! 5. 前缀拥塞 (prefix congestion across variable-length paths)
//! 6. 子树/邻域外部性 (subtree/neighborhood externality on continuations)
//! 7. 分段歧义性 (segmentation ambiguity in continuous typing)
//! 8. XHUP 规则偏离度 (deviation from official/attested XHUP rules)
//! 9. 肌肉记忆迁移成本 (legacy v1/v2 muscle-memory migration penalty)
//!
//! 绝不单独优化 KSPC。所有项显式可解释、可归因。

/// 成本与效用模型参数配置。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrefixCostModel {
    /// 每键基础击键成本。
    pub key_cost: f64,
    /// 候选位选择成本(索引 0 为 rank 1, 超出使用末尾)。
    pub rank_cost: [f64; 4],
    /// 词汇域全局频率权重。
    pub lexical_weight: f64,
    /// 会话域局部频率权重。
    pub conversation_weight: f64,
    /// 码位碰撞与占用竞争系数。
    pub collision_coeff: f64,
    /// 前缀拥塞惩罚系数。
    pub congestion_coeff: f64,
    /// 子树/邻域外部性惩罚系数。
    pub externality_coeff: f64,
    /// 分段歧义惩罚系数。
    pub ambiguity_coeff: f64,
    /// XHUP 规则偏离惩罚系数。
    pub rule_deviation_coeff: f64,
    /// 肌肉记忆迁移成本系数(保护既有 v1/v2 习惯)。
    pub migration_coeff: f64,
}

impl Default for PrefixCostModel {
    fn default() -> Self {
        Self {
            key_cost: 1.0,
            rank_cost: [0.0, 0.5, 1.0, 1.8],
            lexical_weight: 0.6,
            conversation_weight: 0.4,
            collision_coeff: 0.4,
            congestion_coeff: 0.3,
            externality_coeff: 0.2,
            ambiguity_coeff: 0.2,
            rule_deviation_coeff: 0.8,
            migration_coeff: 1.2,
        }
    }
}

impl PrefixCostModel {
    /// 计算名次选择基础成本。
    pub fn rank_cost_at(&self, rank: usize) -> f64 {
        let idx = rank.saturating_sub(1).min(self.rank_cost.len() - 1);
        self.rank_cost[idx]
    }
}

/// 放置评估的效用分解(解释卡完整暴露)。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PrefixUtilityBreakdown {
    /// 目标综合频率质量(锚定归一化)。
    pub frequency_mass: f64,
    /// 省键收益 (mass * keys_saved * key_cost)。
    pub keystroke_benefit: f64,
    /// 候选名次选择成本。
    pub rank_cost: f64,
    /// 码位竞争与碰撞成本。
    pub collision_cost: f64,
    /// 前缀拥塞惩罚。
    pub prefix_congestion: f64,
    /// 子树/邻域外部性干扰成本。
    pub externality_cost: f64,
    /// 分段歧义成本。
    pub ambiguity_cost: f64,
    /// 被挤出候选质量造成的扰动。
    pub displaced_mass: f64,
    /// XHUP 规则偏离惩罚。
    pub rule_deviation: f64,
    /// 既有肌肉记忆迁移惩罚。
    pub migration_penalty: f64,
    /// 最终净效用贡献 (收益 - 全部成本)。
    pub net_utility: f64,
}

impl PrefixUtilityBreakdown {
    /// 计算总效用贡献。
    pub fn compute_net(&mut self) -> f64 {
        self.net_utility = self.keystroke_benefit
            - self.rank_cost
            - self.collision_cost
            - self.prefix_congestion
            - self.externality_cost
            - self.ambiguity_cost
            - self.displaced_mass
            - self.rule_deviation
            - self.migration_penalty;
        self.net_utility
    }
}

/// 评估槽位分配的效用与成本分解。
#[allow(clippy::too_many_arguments)]
pub fn evaluate_prefix_placement(
    cost_model: &PrefixCostModel,
    mass: f64,
    full_code_len: usize,
    prefix_len: usize,
    target_rank: usize,
    occupant_mass: f64,
    displaced_mass: f64,
    congestion: f64,
    externality: f64,
    ambiguity: f64,
    rule_deviation: f64,
    migration_penalty: f64,
) -> PrefixUtilityBreakdown {
    let keys_saved = (full_code_len as isize - prefix_len as isize).max(0) as f64;
    let keystroke_benefit = mass * keys_saved * cost_model.key_cost;
    let rank_cost = mass * cost_model.rank_cost_at(target_rank);
    let collision_cost = cost_model.collision_coeff * occupant_mass * mass;
    let prefix_congestion = cost_model.congestion_coeff * congestion;
    let externality_cost = cost_model.externality_coeff * externality;
    let ambiguity_cost = cost_model.ambiguity_coeff * ambiguity;
    let displaced_cost = displaced_mass * cost_model.collision_coeff;
    let rule_cost = cost_model.rule_deviation_coeff * rule_deviation * mass;
    let migration_cost = cost_model.migration_coeff * migration_penalty * mass;

    let mut breakdown = PrefixUtilityBreakdown {
        frequency_mass: mass,
        keystroke_benefit,
        rank_cost,
        collision_cost,
        prefix_congestion,
        externality_cost,
        ambiguity_cost,
        displaced_mass: displaced_cost,
        rule_deviation: rule_cost,
        migration_penalty: migration_cost,
        net_utility: 0.0,
    };
    breakdown.compute_net();
    breakdown
}
