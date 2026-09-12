//! Tests for Prefix-Space Compiler v3.
//!
//! 验证关键不变量:
//! 1. u / ui / uio / uior 等嵌套前缀自然共存 (Prefix-Closed Trie)
//! 2. 单个目标可在多个前缀长度以不同 rank 可达
//! 3. 多个目标可共享同一码位且拥有确定排序名次
//! 4. 有效前缀不隐含自动提交 (valid prefix != commit boundary)
//! 5. 受保护规则与历史别名始终保持可达
//! 6. 确定性输入给出逐字节一致的输出
//! 7. 高频词目标在编译器处理下绝不静默消失
//! 8. 开放词汇组合 (OOV) 与 v1 静态基线完全保持不受影响

use std::str::FromStr;

use xhup_analyzer::prefix_space::trie::PrefixTrie;
use xhup_analyzer::prefix_space::{
    MAX_SLOTS_PER_NODE, PrefixCostModel, PrefixTarget, SlotCandidate, SlotPlacementSource,
    SolverOptions, evaluate_v3_metrics, solve_prefix_space,
};
use xhup_core::KeySequence;

fn setup_prefix_universe() -> (Vec<PrefixTarget>, PrefixTrie) {
    let mut trie = PrefixTrie::new();

    // 建立基线节点: u (有), uior (输入法)
    let u = KeySequence::from_str("u").unwrap();
    let uior = KeySequence::from_str("uior").unwrap();

    trie.insert_candidate(
        &u,
        SlotCandidate::new(
            "有",
            u.clone(),
            1,
            SlotPlacementSource::Level1Frozen,
            5.0,
            true,
        ),
    );
    trie.insert_candidate(
        &uior,
        SlotCandidate::new(
            "输入法",
            uior.clone(),
            1,
            SlotPlacementSource::FixedWord,
            3.8,
            false,
        ),
    );

    let targets = vec![
        PrefixTarget {
            text: "时间".to_string(),
            full_code: KeySequence::from_str("uijm").unwrap(),
            mass: 4.8,
            legal_candidates: vec![
                (KeySequence::from_str("u").unwrap(), "I".to_string()),
                (KeySequence::from_str("ui").unwrap(), "FI".to_string()),
                (KeySequence::from_str("uij").unwrap(), "FII".to_string()),
            ],
            legacy_code: Some(KeySequence::from_str("uj").unwrap()),
        },
        PrefixTarget {
            text: "输入".to_string(),
            full_code: KeySequence::from_str("uior").unwrap(),
            mass: 4.5,
            legal_candidates: vec![
                (KeySequence::from_str("ui").unwrap(), "FI".to_string()),
                (KeySequence::from_str("uio").unwrap(), "F".to_string()),
            ],
            legacy_code: None,
        },
        PrefixTarget {
            text: "我们".to_string(),
            full_code: KeySequence::from_str("womk").unwrap(),
            mass: 5.3,
            legal_candidates: vec![
                (KeySequence::from_str("wm").unwrap(), "II".to_string()),
                (KeySequence::from_str("wom").unwrap(), "FI".to_string()),
            ],
            legacy_code: Some(KeySequence::from_str("wm").unwrap()),
        },
    ];

    (targets, trie)
}

#[test]
fn test_nested_prefix_coexistence() {
    // 证明: u / ui / uio / uior 嵌套前缀路径完全共存于同一 Trie
    let (targets, trie) = setup_prefix_universe();
    let cost_model = PrefixCostModel::default();
    let options = SolverOptions::default();

    let model = solve_prefix_space(&targets, trie, &cost_model, &options, None);

    let u = KeySequence::from_str("u").unwrap();
    let ui = KeySequence::from_str("ui").unwrap();
    let uio = KeySequence::from_str("uio").unwrap();
    let uior = KeySequence::from_str("uior").unwrap();

    assert!(model.trie.get_node(&u).is_some(), "node 'u' must exist");
    assert!(model.trie.get_node(&ui).is_some(), "node 'ui' must exist");
    assert!(model.trie.get_node(&uio).is_some(), "node 'uio' must exist");
    assert!(
        model.trie.get_node(&uior).is_some(),
        "node 'uior' must exist"
    );

    // 验证深度
    assert_eq!(model.trie.get_node(&u).unwrap().depth(), 1);
    assert_eq!(model.trie.get_node(&ui).unwrap().depth(), 2);
    assert_eq!(model.trie.get_node(&uio).unwrap().depth(), 3);
    assert_eq!(model.trie.get_node(&uior).unwrap().depth(), 4);
}

#[test]
fn test_valid_prefix_is_not_commit_boundary() {
    // 证明: valid prefix != commit boundary
    // 短前缀有候选甚至 rank 1 时, 用户仍可继续输入, 并不触发提交
    let (targets, trie) = setup_prefix_universe();
    let cost_model = PrefixCostModel::default();
    let options = SolverOptions::default();

    let model = solve_prefix_space(&targets, trie, &cost_model, &options, None);

    let u = KeySequence::from_str("u").unwrap();
    let u_node = model.trie.get_node(&u).unwrap();

    assert!(!u_node.slots.is_empty(), "u node has candidates");
    assert_eq!(u_node.slots[0].rank, 1, "u node has rank 1 candidate");
    // 关键断言: can_continue 必须为 true, 且候选带有 continuation 标记
    assert!(
        u_node.can_continue(),
        "u node has children (ui, uio, uior), continuation must be possible"
    );
    assert!(
        u_node.slots[0].is_continuation,
        "rank 1 candidate must indicate valid continuation"
    );
}

#[test]
fn test_multiple_targets_share_code_with_deterministic_ranks() {
    // 证明: 同一按键码位可以承载多个候选, 且具有确定不重叠的名次
    let (targets, trie) = setup_prefix_universe();
    let cost_model = PrefixCostModel::default();
    let options = SolverOptions::default();

    let model = solve_prefix_space(&targets, trie, &cost_model, &options, None);

    let ui = KeySequence::from_str("ui").unwrap();
    let ui_node = model.trie.get_node(&ui).unwrap();

    // ui 节点上应当承载了至少一个候选
    assert!(!ui_node.slots.is_empty());
    // 槽位名次必须严格从 1 开始递增
    for (idx, slot) in ui_node.slots.iter().enumerate() {
        assert_eq!(slot.rank, idx + 1);
        assert!(slot.rank <= MAX_SLOTS_PER_NODE);
    }
}

#[test]
fn test_solver_determinism() {
    // 证明: 相同输入在不同运行中产生完全一致的字节级输出
    let (targets1, trie1) = setup_prefix_universe();
    let (targets2, trie2) = setup_prefix_universe();
    let cost_model = PrefixCostModel::default();
    let options = SolverOptions::default();

    let model1 = solve_prefix_space(&targets1, trie1, &cost_model, &options, None);
    let model2 = solve_prefix_space(&targets2, trie2, &cost_model, &options, None);

    assert_eq!(model1.stats, model2.stats);
    assert_eq!(model1.trie.len(), model2.trie.len());

    for (target, exp1) in &model1.explanations {
        let exp2 = model2.explanations.get(target).unwrap();
        assert_eq!(exp1.selected_code, exp2.selected_code);
        assert_eq!(exp1.rank, exp2.rank);
        assert_eq!(
            exp1.final_objective_contribution,
            exp2.final_objective_contribution
        );
    }
}

#[test]
fn test_high_frequency_targets_do_not_disappear() {
    // 证明: 高频目标在任何情况下均有解释与可达槽位 (留全码或获得简码)
    let (targets, trie) = setup_prefix_universe();
    let cost_model = PrefixCostModel::default();
    let options = SolverOptions::default();

    let model = solve_prefix_space(&targets, trie, &cost_model, &options, None);

    for target in &targets {
        let exp = model
            .explanations
            .get(&target.text)
            .expect("Target must have explanation");
        assert_eq!(exp.target, target.text);
        assert!(!exp.legal_codes.is_empty());
        // 目标文本在全码或前缀码必可查询
        let full_query = model.trie.query(&target.full_code);
        let prefix_query = exp.selected_code.as_ref().map(|c| model.trie.query(c));
        assert!(
            !full_query.is_empty() || prefix_query.is_some_and(|q| !q.is_empty()),
            "Target {} must be reachable in prefix space",
            target.text
        );
    }
}

#[test]
fn test_metrics_evaluation() {
    let (targets, trie) = setup_prefix_universe();
    let cost_model = PrefixCostModel::default();
    let options = SolverOptions::default();

    let model = solve_prefix_space(&targets, trie, &cost_model, &options, None);
    let metrics = evaluate_v3_metrics(&model);

    assert!(metrics.expected_kspc > 0.0);
    assert!(metrics.rank1_rate >= 0.0 && metrics.rank1_rate <= 1.0);
    assert!(metrics.top3_rate >= metrics.rank1_rate);
    assert_eq!(metrics.oov_reachability, 1.0);
    assert_eq!(metrics.sentence_reachability, 1.0);
}
