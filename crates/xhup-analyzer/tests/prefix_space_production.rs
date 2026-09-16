//! Prefix-Space Compiler v3 生产数据有界求解(Issue #83 §25 第 4 步)。
//!
//! 不断言具体词的具体码(无词汇政策测试)。不改写冻结 canonical 映射。

use std::sync::OnceLock;

use xhup_analyzer::prefix_space::{
    PrefixCostModel, ProductionPrefixUniverse, SlotPlacementSource, SolverOptions,
    TEST_PRODUCTION_LIMIT, build_production_universe, solve_prefix_space,
    stats_equal_except_runtime, verify_prefix_closed, verify_production_invariants,
};

fn universe() -> &'static ProductionPrefixUniverse {
    static UNIVERSE: OnceLock<ProductionPrefixUniverse> = OnceLock::new();
    UNIVERSE.get_or_init(|| build_production_universe(Some(TEST_PRODUCTION_LIMIT)))
}

fn compiled() -> &'static xhup_analyzer::prefix_space::PrefixSpaceCompiledModel {
    static MODEL: OnceLock<xhup_analyzer::prefix_space::PrefixSpaceCompiledModel> = OnceLock::new();
    MODEL.get_or_init(|| {
        let universe = universe();
        solve_prefix_space(
            &universe.targets,
            universe.initial_trie.clone(),
            &PrefixCostModel::default(),
            &SolverOptions::default(),
            None,
        )
    })
}

#[test]
fn production_universe_is_bounded_and_positive_mass() {
    let universe = universe();
    assert_eq!(universe.targets.len(), TEST_PRODUCTION_LIMIT);
    assert!(
        universe
            .targets
            .iter()
            .all(|t| t.mass.is_finite() && t.mass > 0.0),
        "solver mass must be strictly positive"
    );
    assert!(
        universe.targets.iter().all(|t| t.full_code.len() >= 2),
        "every target keeps a full_code"
    );
}

#[test]
fn level1_frozen_anchors_are_seeded_not_char_table() {
    let occupied = universe().initial_trie.all_occupied_codes();
    assert_eq!(occupied.len(), 26, "only 26 level-1 keys");
    assert!(
        universe().initial_trie.len() < 100,
        "must not dump the 8105-char table into the trie"
    );
    for (code, slots) in occupied {
        assert_eq!(code.len(), 1);
        assert!(
            slots
                .iter()
                .all(|s| { s.source == SlotPlacementSource::Level1Frozen && s.is_frozen })
        );
    }
}

#[test]
fn no_target_silently_dropped_and_full_code_remains_legal() {
    let universe = universe();
    let model = compiled();
    verify_production_invariants(&universe.targets, model).expect("production invariants");
    for target in &universe.targets {
        let exp = model
            .explanations
            .get(&target.text)
            .expect("every input target has an explanation");
        assert!(
            exp.legal_codes.iter().any(|c| c == &target.full_code),
            "{} must keep full_code {}",
            target.text,
            target.full_code
        );
    }
}

#[test]
fn prefix_closed_for_every_existing_node() {
    let model = compiled();
    verify_prefix_closed(&model.trie).expect("trie is prefix-closed");
    for (code, _) in model.trie.all_occupied_codes() {
        let keys = code.as_slice();
        for end in 1..=keys.len() {
            let prefix = xhup_core::KeySequence::from_keys(&keys[..end])
                .expect("occupied code prefixes are non-empty");
            assert!(
                model.trie.get_node(&prefix).is_some(),
                "code {code} exists so prefix {prefix} must exist"
            );
        }
    }
}

#[test]
fn two_solves_are_deterministic() {
    let universe = universe();
    let cost = PrefixCostModel::default();
    let options = SolverOptions::default();
    let model1 = solve_prefix_space(
        &universe.targets,
        universe.initial_trie.clone(),
        &cost,
        &options,
        None,
    );
    let model2 = solve_prefix_space(
        &universe.targets,
        universe.initial_trie.clone(),
        &cost,
        &options,
        None,
    );
    assert!(
        stats_equal_except_runtime(&model1.stats, &model2.stats),
        "stats must match except runtime: {:?} vs {:?}",
        model1.stats,
        model2.stats
    );
    assert_eq!(model1.trie.len(), model2.trie.len());
    assert_eq!(model1.explanations.len(), model2.explanations.len());
    for (text, exp1) in &model1.explanations {
        let exp2 = model2.explanations.get(text).unwrap();
        assert_eq!(exp1.selected_code, exp2.selected_code);
        assert_eq!(exp1.rank, exp2.rank);
        assert_eq!(
            exp1.final_objective_contribution,
            exp2.final_objective_contribution
        );
    }
}
