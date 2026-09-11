use xhup_analyzer::contextual_benchmark::{BenchmarkRunner, BenchmarkSuite, check_baseline_json};

const FIXTURE: &str = include_str!("../../../data/benchmarks/contextual-v1.json");
const BASELINE: &str = include_str!("../../../data/benchmarks/contextual-baseline-v1.json");

#[test]
fn committed_fixture_matches_foundation_baseline() {
    let suite = BenchmarkSuite::from_json(FIXTURE).unwrap();
    let report = BenchmarkRunner::default().run(&suite);
    assert_eq!(report.case_count, 4);
    assert_eq!(report.development_cases, 2);
    assert_eq!(report.evaluation_cases, 2);
    assert_eq!(report.top1_text_correct, 4);
    assert_eq!(report.top1_path_correct, 2);
    assert_eq!(report.top_k_path_correct, 4);
    assert_eq!(report.contextual_case_count, 4);
    assert_eq!(report.contextual_top1_correct, 2);
    assert_eq!(report.total_complete_paths, 8);
    assert_eq!(report.maximum_complete_paths, 2);
    assert_eq!(report.truncated_cases, 0);
    assert_eq!(report.segmentation_accuracy(), 0.5);
    assert_eq!(report.contextual_disambiguation_accuracy(), 0.5);
    assert!(check_baseline_json(&report, BASELINE).unwrap().is_empty());
}

#[test]
fn same_input_and_edges_can_expect_different_segmentations_by_context() {
    let suite = BenchmarkSuite::from_json(FIXTURE).unwrap();
    for pair in [[0, 1], [2, 3]] {
        let first = &suite.cases()[pair[0]];
        let second = &suite.cases()[pair[1]];
        assert_eq!(
            first.context().composition(),
            second.context().composition()
        );
        assert!(first.lattice().edges() == second.lattice().edges());
        assert_ne!(
            first.context().committed_left(),
            second.context().committed_left()
        );
        assert_ne!(first.expected_edge_ids(), second.expected_edge_ids());
    }
}

#[test]
fn parser_rejects_schema_drift_and_unbounded_configuration() {
    let wrong_schema = FIXTURE.replacen(
        "xhup-contextual-benchmark/v1",
        "xhup-contextual-benchmark/v2",
        1,
    );
    assert!(BenchmarkSuite::from_json(&wrong_schema).is_err());

    let zero_limit = FIXTURE.replacen("\"maxPathsPerCase\": 32", "\"maxPathsPerCase\": 0", 1);
    assert!(BenchmarkSuite::from_json(&zero_limit).is_err());
}

#[test]
fn parser_rejects_duplicate_ids_and_broken_expected_paths() {
    let duplicate = FIXTURE.replacen("graduate-prefix-eval", "research-life-dev", 1);
    assert!(BenchmarkSuite::from_json(&duplicate).is_err());

    let broken = FIXTURE.replacen(
        "{ \"start\": 4, \"end\": 8, \"text\": \"生命\" }",
        "{ \"start\": 5, \"end\": 8, \"text\": \"生命\" }",
        1,
    );
    assert!(BenchmarkSuite::from_json(&broken).is_err());
}
