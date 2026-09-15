// Joint lattice conformance tests consuming fixture semantics from
// data/fixtures/joint-lattice-v1.json.
//
// These tests encode the fixture cases as Rust test data to avoid adding
// serde_json as a dependency. Both Rust and Lua tests verify the same
// semantic invariants.

use std::num::NonZeroUsize;

use xhup_decoder::{
    BaselineScorer, CandidateSource, LatticeBounds, LatticeBuildStats, LatticeFusionBuilder,
    RuntimeContext, Span, rank_paths,
};

/// Helper: add candidates and build, returning lattice + stats.
fn build_fixture(
    raw_input: &str,
    candidates: &[(usize, usize, &str, CandidateSource, u64, u32)],
    bounds: Option<LatticeBounds>,
) -> (xhup_decoder::Lattice, LatticeBuildStats) {
    let input = raw_input.parse().unwrap();
    let bounds = bounds.unwrap_or_default();
    let mut builder = LatticeFusionBuilder::new(input, bounds);
    for &(start, end, text, source, freq, rank) in candidates {
        let span = Span::new(start, end).unwrap();
        builder
            .add_candidate(span, text, source, 0, rank, freq)
            .unwrap();
    }
    builder.build()
}

// ── fixture: nested-prefix-path ──
// u / ui / uio / uior: overlapping prefix spans from position 0.
#[test]
fn fixture_nested_prefix_path() {
    use CandidateSource::*;
    let (lattice, stats) = build_fixture(
        "uior",
        &[
            (0, 1, "去", StaticTable, 580_000, 1),
            (0, 2, "我", StaticTable, 1_200_000, 1),
            (0, 2, "时", FlowTable, 340_000, 1),
            (0, 3, "时间", StaticTable, 266_843, 1),
            // Same (span, text) from two sources → fuse
            (0, 4, "时间", StaticTable, 266_843, 1),
            (0, 4, "时间", FlowTable, 266_843, 1),
            (1, 2, "一", FlowTable, 900_000, 1),
            (2, 3, "哦", FlowTable, 50_000, 1),
            (2, 4, "哦尔", FlowTable, 100, 1),
            (3, 4, "日", FlowTable, 120_000, 1),
        ],
        None,
    );

    // 10 raw candidates → 9 fused (时间 at [0,4) fuses from static+flow)
    assert_eq!(stats.raw_candidates, 10);
    assert_eq!(stats.fused_candidates, 9);
    assert_eq!(stats.total_edges, 9);

    // Verify overlapping spans at position 0
    let outgoing_0 = lattice.outgoing(0).unwrap();
    assert!(
        outgoing_0.len() >= 4,
        "位置 0 应有 ≥4 条出边(u/ui/uio/uior)"
    );

    // Verify complete path exists from 0 to 4
    let paths = lattice.complete_paths(NonZeroUsize::new(16).unwrap());
    assert!(!paths.paths().is_empty(), "应存在至少一条完整路径 0→4");
}

// ── fixture: overlapping-lexical-segmentation ──
// yjjqugmk: 研究+生命 vs 研究生+命
#[test]
fn fixture_overlapping_lexical_segmentation() {
    use CandidateSource::*;
    let (lattice, stats) = build_fixture(
        "yjjqugmk",
        &[
            (0, 4, "研究", StaticTable, 266_843, 1),
            (0, 4, "研究", FlowTable, 266_843, 1),
            (4, 8, "生命", StaticTable, 80_039, 1),
            (4, 8, "生命", FlowTable, 80_039, 1),
            (0, 6, "研究生", FlowTable, 72_173, 1),
            (6, 8, "命", FlowTable, 6_600, 1),
            (0, 2, "研", FlowTable, 5_000, 1),
            (2, 4, "究", FlowTable, 3_000, 1),
            (4, 6, "生", FlowTable, 200_000, 1),
            (6, 8, "命", FlowTable, 6_600, 2),
        ],
        None,
    );

    // 研究 at [0,4) and 生命 at [4,8) each fuse; 命 at [6,8) from two entries fuses
    assert_eq!(stats.raw_candidates, 10);
    assert_eq!(stats.fused_candidates, 7);
    assert_eq!(stats.total_edges, 7);

    // Both segmentations must coexist
    let paths = lattice.complete_paths(NonZeroUsize::new(32).unwrap());
    assert!(
        paths.paths().len() >= 2,
        "至少两条分段: 研究|生命 与 研究生|命"
    );

    let context = RuntimeContext::new("", lattice.input().clone());
    let ranked = rank_paths(
        &BaselineScorer::default(),
        &context,
        &lattice,
        paths.paths(),
    );
    let segmentations: Vec<_> = ranked.iter().map(|p| p.segments().join("|")).collect();

    // "研究|生命" should rank higher (frequency sum > "研究生|命")
    assert!(segmentations.contains(&"研究|生命".to_string()));
    assert!(segmentations.contains(&"研究生|命".to_string()));
}

// ── fixture: duplicate-source-fusion ──
// Same text from static, flow, and user sources on same span
#[test]
fn fixture_duplicate_source_fusion() {
    use CandidateSource::*;
    let (lattice, stats) = build_fixture(
        "womf",
        &[
            (0, 4, "我们", StaticTable, 500_000, 1),
            (0, 4, "我们", FlowTable, 500_000, 1),
            (0, 4, "我们", UserLearned, 10, 1),
            (0, 2, "我", FlowTable, 1_200_000, 1),
            (2, 4, "们", FlowTable, 300_000, 1),
        ],
        None,
    );

    // 我们 at [0,4) from 3 sources → 1 fused edge
    assert_eq!(stats.raw_candidates, 5);
    assert_eq!(stats.fused_candidates, 3);
    assert_eq!(stats.total_edges, 3);

    // Fused candidate should have max_frequency = 500000
    let four_key_edge = lattice
        .edges()
        .iter()
        .find(|e| e.candidate().text() == "我们" && e.span().len() == 4)
        .expect("应存在我们的四键边");
    assert_eq!(four_key_edge.candidate().frequency(), 500_000);
}

// ── fixture: character-primitives ──
// 提示词: single-character 2-key sound primitives for OOV reachability
#[test]
fn fixture_character_primitives() {
    use CandidateSource::*;
    let (lattice, stats) = build_fixture(
        "tiuici",
        &[
            (0, 2, "提", FlowTable, 80_000, 1),
            (2, 4, "示", FlowTable, 60_000, 1),
            (4, 6, "词", FlowTable, 40_000, 1),
            (0, 6, "提示词", SentenceComposition, 0, 1),
        ],
        None,
    );

    assert_eq!(stats.fused_candidates, 4);
    assert_eq!(stats.total_edges, 4);

    // Complete coverage: 0→2→4→6 and 0→6
    let paths = lattice.complete_paths(NonZeroUsize::new(8).unwrap());
    assert!(
        paths.paths().len() >= 2,
        "字符原语路径与句子组合路径均应存在"
    );
}

// ── fixture: static-aliases ──
// jqu → 就是 (attested alias) + character primitives
#[test]
fn fixture_static_aliases() {
    use CandidateSource::*;
    let (lattice, stats) = build_fixture(
        "jqu",
        &[
            (0, 3, "就是", StaticTable, 400_000, 1),
            (0, 2, "就", FlowTable, 350_000, 1),
            (2, 3, "是", FlowTable, 500_000, 1),
        ],
        None,
    );

    assert_eq!(stats.fused_candidates, 3);
    assert_eq!(stats.total_edges, 3);

    // Two paths: [就是] and [就|是]
    let paths = lattice.complete_paths(NonZeroUsize::new(8).unwrap());
    assert_eq!(paths.paths().len(), 2);
}

// ── fixture: bounded-fanout-truncation ──
// Excessive candidates on one span trigger truncation
#[test]
fn fixture_bounded_fanout_truncation() {
    use CandidateSource::*;
    let bounds = LatticeBounds {
        max_candidates_per_span: 2,
        max_outgoing_per_position: 4,
        max_total_edges: 100,
        ..Default::default()
    };
    let (lattice, stats) = build_fixture(
        "ab",
        &[
            (0, 2, "甲", StaticTable, 100, 1),
            (0, 2, "乙", StaticTable, 90, 2),
            (0, 2, "丙", StaticTable, 80, 3),
            (0, 1, "啊", FlowTable, 50_000, 1),
            (1, 2, "吧", FlowTable, 30_000, 1),
        ],
        Some(bounds),
    );

    // max_candidates_per_span=2 truncates one candidate from [0,2)
    assert!(stats.truncated_candidates >= 1);
    assert_eq!(stats.total_edges, 4); // 2 for [0,2) + 1 for [0,1) + 1 for [1,2)

    // Fallback 啊 and 吧 (single-char) should be preserved
    let texts: Vec<_> = lattice
        .edges()
        .iter()
        .map(|e| e.candidate().text().to_string())
        .collect();
    assert!(texts.contains(&"啊".to_string()));
    assert!(texts.contains(&"吧".to_string()));
    // 甲 (single char) should be preserved as fallback
    assert!(texts.contains(&"甲".to_string()));
}

// ── fixture: full-coverage-with-fallback ──
// Every position reachable via fallback
#[test]
fn fixture_full_coverage_with_fallback() {
    use CandidateSource::*;
    let (lattice, _stats) = build_fixture(
        "abcd",
        &[
            (0, 4, "甲乙", StaticTable, 100, 1),
            (0, 2, "甲", FlowTable, 50, 1),
            (2, 4, "乙", FlowTable, 50, 1),
        ],
        None,
    );

    let paths = lattice.complete_paths(NonZeroUsize::new(8).unwrap());
    assert_eq!(paths.paths().len(), 2);
}

// ── fixture: oov-open-composition ──
// 提嗯诶: words not in any dictionary, reachable via character primitives
#[test]
fn fixture_oov_open_composition() {
    use CandidateSource::*;
    let (lattice, _stats) = build_fixture(
        "tiogei",
        &[
            (0, 2, "提", FlowTable, 80_000, 1),
            (2, 4, "嗯", FlowTable, 5_000, 1),
            (4, 6, "诶", FlowTable, 3_000, 1),
        ],
        None,
    );

    // Full coverage: one path 提|嗯|诶
    let paths = lattice.complete_paths(NonZeroUsize::new(8).unwrap());
    assert_eq!(paths.paths().len(), 1);

    let context = RuntimeContext::new("", lattice.input().clone());
    let ranked = rank_paths(
        &BaselineScorer::default(),
        &context,
        &lattice,
        paths.paths(),
    );
    assert_eq!(ranked[0].text(), "提嗯诶");
}

// ── fixture: user-learned ──
// User-learned candidate with low frequency still preserved
#[test]
fn fixture_user_learned_candidate() {
    use CandidateSource::*;
    let (lattice, stats) = build_fixture(
        "womfuijm",
        &[
            (0, 4, "我们", StaticTable, 500_000, 1),
            (4, 8, "时间", StaticTable, 266_843, 1),
            (0, 8, "我们时间", UserLearned, 2, 1),
            (0, 2, "我", FlowTable, 1_200_000, 1),
            (2, 4, "们", FlowTable, 300_000, 1),
            (4, 6, "时", FlowTable, 340_000, 1),
            (6, 8, "间", FlowTable, 100_000, 1),
        ],
        None,
    );

    assert_eq!(stats.fused_candidates, 7);
    assert_eq!(stats.total_edges, 7);

    // User-learned 我们时间 at [0,8) should exist
    let user_learned_edge = lattice
        .edges()
        .iter()
        .find(|e| e.candidate().text() == "我们时间");
    assert!(user_learned_edge.is_some(), "用户学习词应保留");
}

// ── determinism: insertion order independence ──
#[test]
fn fixture_insertion_order_independence() {
    use CandidateSource::*;

    let candidates = vec![
        (0usize, 4usize, "研究", StaticTable, 266_843u64, 1u32),
        (0, 4, "研究", FlowTable, 266_843, 1),
        (4, 8, "生命", StaticTable, 80_039, 1),
        (0, 6, "研究生", FlowTable, 72_173, 1),
        (6, 8, "命", FlowTable, 6_600, 1),
    ];

    let (lattice1, stats1) = build_fixture("yjjqugmk", &candidates, None);

    // Reverse insertion order
    let reversed: Vec<_> = candidates.iter().rev().copied().collect();
    let (lattice2, stats2) = build_fixture("yjjqugmk", &reversed, None);

    assert_eq!(stats1.fused_candidates, stats2.fused_candidates);
    assert_eq!(stats1.total_edges, stats2.total_edges);

    let texts1: Vec<_> = lattice1
        .edges()
        .iter()
        .map(|e| e.candidate().text().to_string())
        .collect();
    let texts2: Vec<_> = lattice2
        .edges()
        .iter()
        .map(|e| e.candidate().text().to_string())
        .collect();
    assert_eq!(texts1, texts2, "边文本序列应与插入顺序无关");
}

// ── no-auto-commit invariant ──
// Building a lattice must NOT introduce auto-commit semantics
#[test]
fn no_auto_commit_from_lattice_construction() {
    use CandidateSource::*;
    let (lattice, _stats) = build_fixture(
        "uijm",
        &[
            // Short prefix candidates at position 0
            (0, 1, "去", StaticTable, 580_000, 1),
            (0, 2, "我", StaticTable, 1_200_000, 1),
            (0, 3, "时间", StaticTable, 266_843, 1),
            (0, 4, "时间", StaticTable, 266_843, 1),
            // Fallback characters
            (1, 2, "一", FlowTable, 900_000, 1),
            (2, 3, "就", FlowTable, 100_000, 1),
            (3, 4, "么", FlowTable, 200_000, 1),
        ],
        None,
    );

    // Multiple outgoing edges at position 0 coexist
    let outgoing_0 = lattice.outgoing(0).unwrap();
    assert!(
        outgoing_0.len() >= 3,
        "valid prefix != commit boundary: 多个前缀 span 应共存"
    );

    // Multiple complete paths should exist
    let paths = lattice.complete_paths(NonZeroUsize::new(16).unwrap());
    assert!(paths.paths().len() >= 2, "多种分段假设应同时可用");
}
