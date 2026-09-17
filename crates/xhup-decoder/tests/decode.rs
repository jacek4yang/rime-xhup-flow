use std::num::NonZeroUsize;

use xhup_decoder::{
    BaselineScorer, BigramModel, CandidateKind, DecodeConfig, EdgeCandidate, FallbackReason,
    KdconvBigramScorer, Lattice, LatticePath, RuntimeContext, ScoredPath, Span, decode_beam,
    decode_beam_adaptive, rank_paths,
};

fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).expect("测试用上限非零")
}

fn edge(text: &str, frequency: u64) -> EdgeCandidate {
    EdgeCandidate::new(text, CandidateKind::HotWord, frequency).unwrap()
}

/// 与 `lattice.rs` 相同的合成歧义：研究|生命 vs 研究生|命。
fn research_life_lattice() -> Lattice {
    let mut lattice = Lattice::new("yjjqugmk".parse().unwrap());
    lattice
        .add_edge(Span::new(0, 4).unwrap(), edge("研究", 266_843))
        .unwrap();
    lattice
        .add_edge(Span::new(4, 8).unwrap(), edge("生命", 80_039))
        .unwrap();
    lattice
        .add_edge(Span::new(0, 6).unwrap(), edge("研究生", 72_173))
        .unwrap();
    lattice
        .add_edge(
            Span::new(6, 8).unwrap(),
            EdgeCandidate::new("命", CandidateKind::Character, 6_600).unwrap(),
        )
        .unwrap();
    lattice
}

fn ambiguous_two_path_lattice() -> Lattice {
    let mut lattice = Lattice::new("abcd".parse().unwrap());
    for (start, end, text) in [(0, 2, "甲"), (0, 4, "甲乙"), (2, 4, "乙")] {
        lattice
            .add_edge(Span::new(start, end).unwrap(), edge(text, 1))
            .unwrap();
    }
    lattice
}

fn assert_same_ranked<B: PartialEq + std::fmt::Debug>(
    left: &[ScoredPath<B>],
    right: &[ScoredPath<B>],
) {
    assert_eq!(left.len(), right.len(), "ranked 长度不同");
    for (i, (a, b)) in left.iter().zip(right).enumerate() {
        assert_eq!(a.path(), b.path(), "path #{i}");
        assert_eq!(a.text(), b.text(), "text #{i}");
        assert_eq!(a.segments(), b.segments(), "segments #{i}");
        assert_eq!(a.score(), b.score(), "score #{i}");
        assert_eq!(a.breakdown(), b.breakdown(), "breakdown #{i}");
    }
}

fn assert_covers_input(lattice: &Lattice, path: &LatticePath) {
    let mut pos = 0usize;
    for &id in path.edge_ids() {
        let edge = lattice.edge(id).expect("path edge 必须属于 lattice");
        assert_eq!(edge.span().start(), pos);
        pos = edge.span().end();
    }
    assert_eq!(pos, lattice.input().len());
    assert!(!path.is_empty());
}

#[test]
fn large_beam_top_path_matches_rank_paths() {
    let lattice = research_life_lattice();
    let context = RuntimeContext::new("这个课题关注", lattice.input().clone());
    let scorer = BaselineScorer::default();
    let paths = lattice.complete_paths(nz(8));
    assert!(!paths.truncated());
    let ranked = rank_paths(&scorer, &context, &lattice, paths.paths());
    assert_eq!(ranked[0].segments().join("|"), "研究|生命");

    let decoded = decode_beam(
        &lattice,
        &context,
        &scorer,
        DecodeConfig::new(nz(8), nz(5), 0),
    );
    assert!(!decoded.truncated());
    assert_eq!(decoded.fallback(), None);
    assert_same_ranked(decoded.ranked(), ranked.as_slice());
}

#[test]
fn beam_width_one_returns_complete_covering_path() {
    let lattice = research_life_lattice();
    let context = RuntimeContext::new("", lattice.input().clone());
    let decoded = decode_beam(
        &lattice,
        &context,
        &BaselineScorer::default(),
        DecodeConfig::new(nz(1), nz(1), 0),
    );
    assert_eq!(decoded.ranked().len(), 1);
    assert_covers_input(&lattice, decoded.ranked()[0].path());
}

#[test]
fn ambiguous_dag_beam_truncated_matches_overflow() {
    let lattice = ambiguous_two_path_lattice();
    let paths = lattice.complete_paths(nz(1));
    assert!(paths.truncated());
    assert_eq!(paths.paths().len(), 1);

    let context = RuntimeContext::new("", lattice.input().clone());
    let scorer = BaselineScorer::default();

    let small = decode_beam(
        &lattice,
        &context,
        &scorer,
        DecodeConfig::new(nz(1), nz(5), 0),
    );
    assert_eq!(small.ranked().len(), 1);
    assert!(small.truncated());
    assert_eq!(small.fallback(), Some(FallbackReason::BeamTruncated));
    assert_covers_input(&lattice, small.ranked()[0].path());

    let large = decode_beam(
        &lattice,
        &context,
        &scorer,
        DecodeConfig::new(nz(8), nz(5), 0),
    );
    assert!(!large.truncated());
    assert_eq!(large.fallback(), None);
    assert_eq!(large.ranked().len(), 2);
}

#[test]
fn decode_beam_is_deterministic() {
    let lattice = research_life_lattice();
    let context = RuntimeContext::new("这个课题关注", lattice.input().clone());
    let scorer = BaselineScorer::default();
    let config = DecodeConfig::default();
    let a = decode_beam(&lattice, &context, &scorer, config);
    let b = decode_beam(&lattice, &context, &scorer, config);
    assert!(a == b, "相同输入下 decode_beam 必须得到相同结果");
}

#[test]
fn runtime_context_debug_still_redacts_user_text() {
    let context = RuntimeContext::new("这个课题关注", "yjjqugmk".parse().unwrap());
    let debug = format!("{context:?}");
    assert!(debug.contains("committed_left_chars: 6"));
    assert!(debug.contains("composition_keys: 8"));
    assert!(!debug.contains("课题"));
    assert!(!debug.contains("yjjq"));
}

#[test]
fn empty_or_incomplete_lattice_returns_empty_ranked() {
    let scorer = BaselineScorer::default();
    let empty = Lattice::new("abcd".parse().unwrap());
    let context = RuntimeContext::new("", empty.input().clone());
    let decoded = decode_beam(&empty, &context, &scorer, DecodeConfig::default());
    assert!(decoded.ranked().is_empty());
    assert!(!decoded.truncated());
    assert_eq!(decoded.fallback(), None);

    let mut incomplete = Lattice::new("abcd".parse().unwrap());
    incomplete
        .add_edge(Span::new(0, 2).unwrap(), edge("甲", 1))
        .unwrap();
    let context = RuntimeContext::new("", incomplete.input().clone());
    let decoded = decode_beam(&incomplete, &context, &scorer, DecodeConfig::default());
    assert!(decoded.ranked().is_empty());
    assert!(!decoded.truncated());
    assert_eq!(decoded.fallback(), None);
}

#[test]
fn incomplete_overflow_still_reports_empty_not_truncated() {
    let mut lattice = Lattice::new("abcd".parse().unwrap());
    for i in 0..10 {
        let text = format!("甲{i}");
        lattice
            .add_edge(Span::new(0, 2).unwrap(), edge(&text, 1))
            .unwrap();
    }
    let context = RuntimeContext::new("", lattice.input().clone());
    let decoded = decode_beam(
        &lattice,
        &context,
        &BaselineScorer::default(),
        DecodeConfig::new(nz(2), nz(5), 0),
    );
    assert!(decoded.ranked().is_empty());
    assert!(!decoded.truncated());
    assert_eq!(decoded.fallback(), None);
}

#[test]
fn low_confidence_sets_fallback_without_changing_rank() {
    let lattice = research_life_lattice();
    let context = RuntimeContext::new("这个课题关注", lattice.input().clone());
    let scorer = BaselineScorer::default();
    let ranked = decode_beam(
        &lattice,
        &context,
        &scorer,
        DecodeConfig::new(nz(8), nz(5), 0),
    );
    let flagged = decode_beam(
        &lattice,
        &context,
        &scorer,
        DecodeConfig::new(nz(8), nz(5), 1_000_000),
    );
    assert_same_ranked(ranked.ranked(), flagged.ranked());
    assert_eq!(ranked.fallback(), None);
    assert_eq!(flagged.fallback(), Some(FallbackReason::LowConfidence));
    assert!(!flagged.truncated());
}

#[test]
fn truncated_fallback_takes_priority_over_low_confidence() {
    let lattice = research_life_lattice();
    let context = RuntimeContext::new("", lattice.input().clone());
    let decoded = decode_beam(
        &lattice,
        &context,
        &BaselineScorer::default(),
        DecodeConfig::new(nz(1), nz(5), 1_000_000),
    );
    assert!(decoded.truncated());
    assert_eq!(decoded.fallback(), Some(FallbackReason::BeamTruncated));
}

#[test]
fn oov_open_composition_survives_low_confidence() {
    let mut lattice = Lattice::new("abcd".parse().unwrap());
    lattice
        .add_edge(Span::new(0, 2).unwrap(), edge("甲", 100))
        .unwrap();
    lattice
        .add_edge(Span::new(2, 4).unwrap(), edge("乙", 100))
        .unwrap();
    lattice
        .add_edge(
            Span::new(0, 4).unwrap(),
            EdgeCandidate::new("甲乙", CandidateKind::OovComposition, 0).unwrap(),
        )
        .unwrap();
    let context = RuntimeContext::new("", lattice.input().clone());
    let decoded = decode_beam(
        &lattice,
        &context,
        &BaselineScorer::default(),
        DecodeConfig::new(nz(8), nz(5), 1_000_000),
    );
    assert_eq!(decoded.fallback(), Some(FallbackReason::LowConfidence));
    assert!(
        decoded
            .ranked()
            .iter()
            .any(|path| path.segments() == ["甲乙"]
                && path.path().edge_ids().iter().any(|&id| {
                    lattice.edge(id).unwrap().candidate().kind() == CandidateKind::OovComposition
                })),
        "低置信回退不得丢弃 lattice 中的 OOV/open-composition 路径"
    );
}

#[test]
fn decode_beam_rescores_survivors_with_provided_scorer() {
    let lattice = research_life_lattice();
    let mut model = BigramModel::default();
    // 证据强度须与标定后的 transition_weight = 2 相称(见 bigram.rs 标定文档):
    // 10 次共现只在旧的 256 权重下才够翻转约 5659 Q10 的词频差。
    model.observe("<s>", "研究生", 50);
    model.observe("研究生", "命", 50);
    model.observe("研究", "生命", 3);
    let scorer = KdconvBigramScorer::new(model);
    let context = RuntimeContext::new("他是研究生", lattice.input().clone());
    let paths = lattice.complete_paths(nz(32));
    let ranked = rank_paths(&scorer, &context, &lattice, paths.paths());
    let decoded = decode_beam(
        &lattice,
        &context,
        &scorer,
        DecodeConfig::new(nz(8), nz(5), 0),
    );
    assert_same_ranked(decoded.ranked(), ranked.as_slice());
    assert_eq!(decoded.ranked()[0].segments(), ["研究生", "命"]);
}

// ---------------------------------------------------------------------------
// 自适应扩宽：在 [beam_width, max_width] 内保证与穷举排序等价
// ---------------------------------------------------------------------------

/// 构造一个**扇出很大**的 lattice：一个位置上有 n 个候选，全部直达末尾。
/// 用于验证窄 beam 会截断、自适应扩宽能恢复等价。
fn wide_fanout_lattice(n: usize) -> Lattice {
    let mut lattice = Lattice::new("ab".parse().unwrap());
    let span = Span::new(0, 2).unwrap();
    for i in 0..n {
        // 频率递增，使排序唯一确定、便于断言。
        lattice
            .add_edge(span, edge(&format!("c{i:03}"), (i as u64 + 1) * 10))
            .unwrap();
    }
    lattice
}

#[test]
fn adaptive_decode_widens_until_exhaustive_equivalence() {
    let lattice = wide_fanout_lattice(20);
    let context = RuntimeContext::new("", lattice.input().clone());
    let scorer = BaselineScorer::default();
    let config = DecodeConfig::new(nz(2), nz(5), 0);

    // 窄 beam 必然截断 —— 先确认前提成立。
    let narrow = decode_beam(&lattice, &context, &scorer, config);
    assert!(narrow.truncated(), "扇出 20 > beam 2，必须截断");

    // 自适应扩宽到 32 应当覆盖扇出 20。宽度按 2 -> 4 -> 8 -> 16 -> 32 倍增，
    // 直到某一档不再截断（16 仍小于扇出 20，故最终落在 32）。
    let (decoded, outcome) = decode_beam_adaptive(&lattice, &context, &scorer, config, nz(32));
    assert!(!outcome.truncated_at_max, "beam 32 足以覆盖扇出 20");
    assert_eq!(
        outcome.used_width, 32,
        "2->4->8->16 仍截断，32 才覆盖扇出 20"
    );
    assert!(!decoded.truncated(), "扩宽后不应再截断");

    // 与穷举排序逐路径一致(top_k=5)。
    let paths = lattice.complete_paths(nz(64));
    assert!(!paths.truncated());
    let ranked = rank_paths(&scorer, &context, &lattice, paths.paths());
    assert_same_ranked(decoded.ranked(), &ranked[..5]);
}

#[test]
fn adaptive_decode_reports_truncation_when_ceiling_is_insufficient() {
    let lattice = wide_fanout_lattice(40);
    let context = RuntimeContext::new("", lattice.input().clone());
    let scorer = BaselineScorer::default();
    let config = DecodeConfig::new(nz(2), nz(5), 0);

    let (decoded, outcome) = decode_beam_adaptive(&lattice, &context, &scorer, config, nz(16));
    assert_eq!(outcome.used_width, 16, "必须顶到上限");
    assert!(outcome.truncated_at_max, "扇出 40 > 上限 16，必须诚实报告");
    assert!(decoded.truncated());
    assert_eq!(decoded.fallback(), Some(FallbackReason::BeamTruncated));
}

#[test]
fn adaptive_decode_matches_plain_decode_when_no_truncation() {
    // 宽 beam 下自适应分支不得改变任何结果（纯等价性回归守卫）。
    let lattice = research_life_lattice();
    let context = RuntimeContext::new("这个课题关注", lattice.input().clone());
    let scorer = KdconvBigramScorer::new(BigramModel::default());
    let config = DecodeConfig::new(nz(32), nz(5), 0);

    let plain = decode_beam(&lattice, &context, &scorer, config);
    let (adaptive, outcome) = decode_beam_adaptive(&lattice, &context, &scorer, config, nz(32));
    assert_eq!(outcome.used_width, 32, "一开始就不截断则不得扩宽");
    assert!(!outcome.truncated_at_max);
    assert_same_ranked(plain.ranked(), adaptive.ranked());
}

#[test]
fn adaptive_decode_never_narrows_below_floor_or_exceeds_ceiling() {
    // 上限小于起始宽度时，实际宽度不得低于 floor。
    let lattice = wide_fanout_lattice(4);
    let context = RuntimeContext::new("", lattice.input().clone());
    let scorer = BaselineScorer::default();
    let config = DecodeConfig::new(nz(8), nz(5), 0);
    let (_, outcome) = decode_beam_adaptive(&lattice, &context, &scorer, config, nz(2));
    assert!(outcome.used_width >= 8, "不得低于 beam_width");
    assert_eq!(outcome.used_width, 8, "上限被 floor 抬升");
}

#[test]
fn adaptive_decode_is_deterministic() {
    let lattice = wide_fanout_lattice(20);
    let context = RuntimeContext::new("", lattice.input().clone());
    let scorer = BaselineScorer::default();
    let config = DecodeConfig::new(nz(2), nz(5), 0);
    let (a, ao) = decode_beam_adaptive(&lattice, &context, &scorer, config, nz(32));
    let (b, bo) = decode_beam_adaptive(&lattice, &context, &scorer, config, nz(32));
    assert_eq!(ao, bo);
    assert_same_ranked(a.ranked(), b.ranked());
}

#[test]
fn adaptive_decode_empty_lattice_is_empty_not_panic() {
    let lattice = Lattice::new("ab".parse().unwrap());
    let context = RuntimeContext::new("", lattice.input().clone());
    let scorer = BaselineScorer::default();
    let config = DecodeConfig::new(nz(2), nz(5), 0);
    let (decoded, outcome) = decode_beam_adaptive(&lattice, &context, &scorer, config, nz(32));
    assert!(decoded.ranked().is_empty());
    assert!(!decoded.truncated());
    assert!(!outcome.truncated_at_max);
    assert_eq!(decoded.fallback(), None);
}
