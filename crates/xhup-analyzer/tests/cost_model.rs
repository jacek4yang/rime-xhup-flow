//! 成本模型测试:选择/歧义/模式复杂度的定义行为。

use xhup_analyzer::cost::CostModel;

#[test]
fn selection_cost_steps() {
    let cost = CostModel::balanced();
    assert_eq!(cost.selection_cost(1), 0.0);
    assert_eq!(cost.selection_cost(2), 1.0);
    assert_eq!(cost.selection_cost(9), 1.0);
    assert_eq!(cost.selection_cost(10), 2.0);
    assert_eq!(cost.selection_cost(100), 2.0);
}

#[test]
fn ambiguity_cost_is_zero_for_single_candidate() {
    let cost = CostModel::balanced();
    assert_eq!(cost.ambiguity_cost(0), 0.0, "空码无歧义");
    assert_eq!(cost.ambiguity_cost(1), 0.0, "唯一候选无歧义");
    assert_eq!(cost.ambiguity_cost(2), 0.5);
    assert_eq!(cost.ambiguity_cost(4), 1.0);
    assert_eq!(cost.ambiguity_cost(8), 1.5);
}

#[test]
fn mode_complexity_counts_transitions() {
    let words = xhup_generator::word_code_analysis_entries();
    let (targets, _) = xhup_analyzer::candidates::enumerate_targets(&words);
    let find = |pattern: &str| {
        targets
            .iter()
            .flat_map(|t| t.candidates())
            .find(|c| c.mode().pattern() == pattern)
            .unwrap_or_else(|| panic!("应存在 {pattern} 模式候选"))
            .mode()
            .clone()
    };
    let balanced = CostModel::balanced();
    assert_eq!(balanced.mode_complexity_cost(&find("FI")), 0.0, "默认为 0");
    let penalized = CostModel {
        mode_complexity_per_transition: 0.1,
        ..CostModel::balanced()
    };
    assert!((penalized.mode_complexity_cost(&find("FI")) - 0.1).abs() < 1e-9);
    assert!((penalized.mode_complexity_cost(&find("FIF")) - 0.2).abs() < 1e-9);
    assert!((penalized.mode_complexity_cost(&find("FII")) - 0.1).abs() < 1e-9);
}
