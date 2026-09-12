//! Deterministic Global Solver: 前缀空间候选槽位全局优化求解器。
//!
//! 求解策略:
//! - 确定性槽位分配 + 迭代重加权 (Iterative Reweighted Global Assignment)
//! - 兼顾候选位容量约束、前缀子树拥塞、同码碰撞熵与迁移成本
//! - 目标可在多个前缀长度以不同 rank 可达 (例如 u rank 2, ui rank 1, uior exact)
//! - 严格保证:
//!   1. 全码可达性恒定 (词典不决定语言可达性)
//!   2. 静态基线与受保护官方规则不被破坏
//!   3. 浮点比较与多键 tie-break 完全确定, 同输入必得同输出

use std::collections::BTreeMap;
use xhup_core::KeySequence;
use xhup_core::rules::{CompatibilityClass, RuleFixture};

use super::cost::{PrefixCostModel, PrefixUtilityBreakdown, evaluate_prefix_placement};
use super::explain::PrefixPlacementExplanation;
use super::slot::{SlotCandidate, SlotPlacementSource};
use super::trie::PrefixTrie;

/// 单节点最大容量(页面常用显示槽位数)。
pub const MAX_SLOTS_PER_NODE: usize = 5;

/// 优化求解的目标实体。
#[derive(Clone, Debug)]
pub struct PrefixTarget {
    /// 目标文本 (词或字)。
    pub text: String,
    /// 规范全码。
    pub full_code: KeySequence,
    /// 综合频率质量(锚定归一化)。
    pub mass: f64,
    /// 结构合法的前缀/简码序列列表及其模式描述 (例如 "FI", "II")。
    pub legal_candidates: Vec<(KeySequence, String)>,
    /// 历史/既有已建立的简码(若有, 用于计算迁移惩罚)。
    pub legacy_code: Option<KeySequence>,
}

/// 编译后的前缀空间全局产物。
#[derive(Clone, Debug)]
pub struct PrefixSpaceCompiledModel {
    /// 前缀闭合码空间 Trie。
    pub trie: PrefixTrie,
    /// 目标放置决策解释映射: 目标文本 → 详细解释卡。
    pub explanations: BTreeMap<String, PrefixPlacementExplanation>,
    /// 全局聚合统计。
    pub stats: PrefixSpaceStats,
}

/// 前缀空间全局统计指标。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PrefixSpaceStats {
    /// 参与优化的目标总数。
    pub total_targets: usize,
    /// 前缀树中存在的节点总数。
    pub total_nodes: usize,
    /// 包含候选的码序列数(前缀空间利用率分子)。
    pub occupied_codes: usize,
    /// 放置的候选总槽位数。
    pub total_slot_assignments: usize,
    /// 首选(rank 1)覆盖的目标数。
    pub rank1_targets: usize,
    /// 前三候选(rank 1..=3)覆盖的目标数。
    pub top3_targets: usize,
    /// 频率加权平均候选位。
    pub weighted_rank: f64,
    /// 预期加权击键数(KSPC 估算)。
    pub expected_kspc: f64,
    /// 码位碰撞熵(碰撞均匀与稀疏程度指标)。
    pub collision_entropy: f64,
    /// 迁移成本总计。
    pub total_migration_penalty: f64,
    /// 求解运行耗时(毫秒)。
    pub solver_time_ms: u64,
}

/// 求解器运行配置。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolverOptions {
    /// 迭代重加权轮数(默认 3 轮已能充分收敛)。
    pub reweight_iterations: usize,
    /// 每个目标允许的最大放置前缀数(1 表示只放最佳简码, 2 允许在短/长前缀双阶梯可达)。
    pub max_placements_per_target: usize,
}

impl Default for SolverOptions {
    fn default() -> Self {
        Self {
            reweight_iterations: 3,
            max_placements_per_target: 2,
        }
    }
}

/// 评估候选码对于指定目标的 XHUP 规则偏离度。
fn compute_rule_deviation(
    target: &PrefixTarget,
    code: &KeySequence,
    rules: Option<&[RuleFixture<'_>]>,
) -> f64 {
    // 1. 若码与全码完全一致, 偏离度为 0
    if code == &target.full_code {
        return 0.0;
    }

    // 2. 检查是否匹配官方规则夹具
    if let Some(rf) = rules {
        let matched = rf
            .iter()
            .find(|f| f.text == target.text && f.expected.as_ref() == Some(code));
        if let Some(fixture) = matched {
            return match fixture.class {
                CompatibilityClass::ExactOfficial
                | CompatibilityClass::OfficialAlias
                | CompatibilityClass::OfficialSpecial => 0.0,
                CompatibilityClass::HistoricalCompatible | CompatibilityClass::RuleCompatible => {
                    0.15
                }
                CompatibilityClass::FlowExtension => 0.35,
                _ => 0.8,
            };
        }
    }

    // 3. 结构单调前缀 (F* I*), 视作规范 Flow 扩展
    0.3
}

/// 评估肌肉记忆迁移惩罚。
fn compute_migration_penalty(target: &PrefixTarget, code: &KeySequence) -> f64 {
    match &target.legacy_code {
        Some(legacy) if legacy == code => 0.0, // 完美保留既有习惯
        Some(_) => 0.8,                        // 改变了既有简码习惯
        None => 0.0,                           // 以前无简码, 无迁移成本
    }
}

/// 候选放置候选提议。
struct PlacementProposal {
    target_idx: usize,
    code: KeySequence,
    pattern: String,
    estimated_rank: usize,
    breakdown: PrefixUtilityBreakdown,
}

/// 确定性全局求解器主入口。
pub fn solve_prefix_space(
    targets: &[PrefixTarget],
    initial_trie: PrefixTrie,
    cost_model: &PrefixCostModel,
    options: &SolverOptions,
    rules: Option<&[RuleFixture<'_>]>,
) -> PrefixSpaceCompiledModel {
    let start_time = std::time::Instant::now();
    let mut trie = initial_trie;
    trie.recompute_subtree_stats();

    // 预建每个目标的合法码列表
    let mut explanations: BTreeMap<String, PrefixPlacementExplanation> = BTreeMap::new();
    for t in targets {
        let mut legal = Vec::with_capacity(t.legal_candidates.len() + 1);
        for (c, _) in &t.legal_candidates {
            if !legal.contains(c) {
                legal.push(c.clone());
            }
        }
        if !legal.contains(&t.full_code) {
            legal.push(t.full_code.clone());
        }
        legal.sort();

        explanations.insert(
            t.text.clone(),
            PrefixPlacementExplanation {
                target: t.text.clone(),
                full_code: t.full_code.clone(),
                legal_codes: legal,
                selected_code: None,
                rank: 1,
                frequency_mass: t.mass,
                keystroke_benefit: 0.0,
                collision_cost: 0.0,
                prefix_congestion: 0.0,
                displaced_mass: 0.0,
                rule_deviation: 0.0,
                migration_penalty: 0.0,
                final_objective_contribution: 0.0,
                rationale: "Retained at full code".to_string(),
            },
        );
    }

    // 迭代重加权多轮分配
    for _iter in 0..options.reweight_iterations {
        let mut proposals: Vec<PlacementProposal> = Vec::new();

        for (target_idx, target) in targets.iter().enumerate() {
            for (code, pattern) in &target.legal_candidates {
                // 不为全码自身重复评估简码节省
                if code == &target.full_code {
                    continue;
                }

                let node_opt = trie.get_node(code);
                let occupant_mass = node_opt.map_or(0.0, |n| n.direct_mass);
                let congestion =
                    node_opt.map_or(0.0, |n| n.subtree_mass / (1.0 + code.len() as f64));
                let externality = node_opt.map_or(0.0, |n| n.continuation_paths as f64);
                let ambiguity = if code.len() <= 2 { 0.4 } else { 0.1 };

                // 预估插入名次
                let estimated_rank = match node_opt {
                    Some(n) => {
                        let rank = n
                            .slots
                            .iter()
                            .position(|s| s.mass < target.mass)
                            .map_or(n.slots.len() + 1, |p| p + 1);
                        rank.min(MAX_SLOTS_PER_NODE + 1)
                    }
                    None => 1,
                };

                if estimated_rank > MAX_SLOTS_PER_NODE {
                    continue; // 超过容量上限
                }

                let displaced_mass = node_opt.map_or(0.0, |n| {
                    n.slots
                        .iter()
                        .skip(estimated_rank.saturating_sub(1))
                        .map(|s| s.mass)
                        .sum::<f64>()
                });

                let rule_dev = compute_rule_deviation(target, code, rules);
                let mig_pen = compute_migration_penalty(target, code);

                let breakdown = evaluate_prefix_placement(
                    cost_model,
                    target.mass,
                    target.full_code.len(),
                    code.len(),
                    estimated_rank,
                    occupant_mass,
                    displaced_mass,
                    congestion,
                    externality,
                    ambiguity,
                    rule_dev,
                    mig_pen,
                );

                if breakdown.net_utility > 0.0 {
                    proposals.push(PlacementProposal {
                        target_idx,
                        code: code.clone(),
                        pattern: pattern.clone(),
                        estimated_rank,
                        breakdown,
                    });
                }
            }
        }

        // 确定性全序排序: 净效用降序 → 质量降序 → 目标文本字典序 → 码字典序
        proposals.sort_by(|a, b| {
            b.breakdown
                .net_utility
                .total_cmp(&a.breakdown.net_utility)
                .then(
                    targets[b.target_idx]
                        .mass
                        .total_cmp(&targets[a.target_idx].mass),
                )
                .then(targets[a.target_idx].text.cmp(&targets[b.target_idx].text))
                .then(a.code.cmp(&b.code))
        });

        // 槽位接纳与分配
        let mut target_placement_counts = vec![0usize; targets.len()];
        let mut node_slot_counts: BTreeMap<KeySequence, usize> = BTreeMap::new();

        // 统计初始既有冻结槽位数
        for (code, slots) in trie.all_occupied_codes() {
            let frozen_count = slots.iter().filter(|s| s.is_frozen).count();
            if frozen_count > 0 {
                node_slot_counts.insert(code, frozen_count);
            }
        }

        for p in proposals {
            if target_placement_counts[p.target_idx] >= options.max_placements_per_target {
                continue;
            }
            let current_node_slots = *node_slot_counts.get(&p.code).unwrap_or(&0);
            if current_node_slots >= MAX_SLOTS_PER_NODE {
                continue;
            }

            let target = &targets[p.target_idx];
            let candidate = SlotCandidate::new(
                target.text.clone(),
                target.full_code.clone(),
                p.estimated_rank,
                SlotPlacementSource::FlowExtension,
                target.mass,
                false, // insert_candidate 会重新按子树更新
            );

            trie.insert_candidate(&p.code, candidate);
            target_placement_counts[p.target_idx] += 1;
            *node_slot_counts.entry(p.code.clone()).or_default() += 1;

            // 记录最佳解释(优先采纳净效用最高者)
            if let Some(exp) = explanations.get_mut(&target.text) {
                let is_better = exp.selected_code.is_none()
                    || p.breakdown.net_utility > exp.final_objective_contribution;
                if is_better {
                    exp.selected_code = Some(p.code.clone());
                    exp.rank = p.estimated_rank;
                    exp.keystroke_benefit = p.breakdown.keystroke_benefit;
                    exp.collision_cost = p.breakdown.collision_cost;
                    exp.prefix_congestion = p.breakdown.prefix_congestion;
                    exp.displaced_mass = p.breakdown.displaced_mass;
                    exp.rule_deviation = p.breakdown.rule_deviation;
                    exp.migration_penalty = p.breakdown.migration_penalty;
                    exp.final_objective_contribution = p.breakdown.net_utility;
                    exp.rationale = format!(
                        "Assigned to prefix {} with net utility +{:.4} (pattern {})",
                        p.code, p.breakdown.net_utility, p.pattern
                    );
                }
            }
        }

        // 重新聚合整树统计
        trie.recompute_subtree_stats();
    }

    // 最终回填实际 rank
    for exp in explanations.values_mut() {
        let rank = exp
            .selected_code
            .as_ref()
            .and_then(|c| trie.get_node(c))
            .and_then(|n| n.find_rank(&exp.target));
        if let Some(actual_rank) = rank {
            exp.rank = actual_rank;
        }
    }

    // 聚合统计指标
    let occupied = trie.all_occupied_codes();
    let mut stats = PrefixSpaceStats {
        total_targets: targets.len(),
        total_nodes: trie.len(),
        occupied_codes: occupied.len(),
        ..Default::default()
    };

    let mut weighted_rank_sum = 0.0;
    let mut total_mass_sum = 0.0;
    let mut weighted_kspc_sum = 0.0;
    let mut slot_counts_entropy = Vec::new();

    for target in targets {
        let exp = explanations.get(&target.text).unwrap();
        let (effective_len, effective_rank) = match &exp.selected_code {
            Some(code) => (code.len(), exp.rank),
            None => (target.full_code.len(), 1),
        };

        if effective_rank == 1 {
            stats.rank1_targets += 1;
        }
        if effective_rank <= 3 {
            stats.top3_targets += 1;
        }

        weighted_rank_sum += target.mass * effective_rank as f64;
        let select_keys = if effective_rank == 1 { 0.0 } else { 1.0 };
        weighted_kspc_sum += target.mass * (effective_len as f64 + select_keys);
        total_mass_sum += target.mass;
        stats.total_migration_penalty += exp.migration_penalty;
    }

    for (_, slots) in &occupied {
        stats.total_slot_assignments += slots.len();
        slot_counts_entropy.push(slots.len() as f64);
    }

    if total_mass_sum > 0.0 {
        stats.weighted_rank = weighted_rank_sum / total_mass_sum;
        stats.expected_kspc = weighted_kspc_sum / total_mass_sum;
    }

    // 计算码位槽位分布熵
    let total_slots_f = stats.total_slot_assignments as f64;
    if total_slots_f > 0.0 {
        let mut entropy = 0.0;
        for c in slot_counts_entropy {
            let p = c / total_slots_f;
            if p > 0.0 {
                entropy -= p * p.ln();
            }
        }
        stats.collision_entropy = entropy;
    }

    stats.solver_time_ms = start_time.elapsed().as_millis() as u64;

    PrefixSpaceCompiledModel {
        trie,
        explanations,
        stats,
    }
}
