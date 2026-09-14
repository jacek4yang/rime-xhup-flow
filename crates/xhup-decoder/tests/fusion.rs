use xhup_decoder::{CandidateEvidence, CandidateKind, EdgeCandidate, Lattice, Span};

fn cand(text: &str, kind: CandidateKind, frequency: u64) -> EdgeCandidate {
    EdgeCandidate::new(text, kind, frequency).unwrap()
}

#[test]
fn same_span_same_text_fuses_into_single_edge_with_merged_evidence() {
    // 同一 (span, text) 来自两个来源:hot 词典 + 扩展词库。
    let mut lattice = Lattice::new("abcd".parse().unwrap());
    let first = lattice
        .add_edge(
            Span::new(0, 4).unwrap(),
            cand("甲乙", CandidateKind::HotWord, 100),
        )
        .unwrap();
    let second = lattice
        .add_edge(
            Span::new(0, 4).unwrap(),
            cand("甲乙", CandidateKind::ExtendedWord, 80),
        )
        .unwrap();

    // deterministic fusion:不制造重复语义路径,保留首插 EdgeId。
    assert_eq!(first, second);
    assert_eq!(lattice.edges().len(), 1);
    let merged = lattice.edge(first).unwrap().candidate();
    assert_eq!(merged.kind(), CandidateKind::HotWord);
    assert_eq!(merged.frequency(), 100);
    let evidence = merged.evidence();
    assert_eq!(evidence.len(), 2);
    assert_eq!(evidence[0].kind(), CandidateKind::HotWord);
    assert_eq!(evidence[0].frequency(), 100);
    assert_eq!(evidence[1].kind(), CandidateKind::ExtendedWord);
    assert_eq!(evidence[1].frequency(), 80);
}

#[test]
fn fusion_dedupes_identical_evidence_and_is_insertion_order_independent() {
    // 三次重复来源 + 顺序差异 → 融合结果与插入顺序无关。
    let build = |order: u32| {
        let mut lattice = Lattice::new("abcd".parse().unwrap());
        let ab = (
            CandidateEvidence::new(CandidateKind::ExtendedWord, 80),
            "甲乙",
        );
        let cd = (CandidateEvidence::new(CandidateKind::HotWord, 100), "甲乙");
        let seq: Vec<(CandidateEvidence, &str)> = if order == 0 {
            vec![ab, cd, cd, ab]
        } else {
            vec![cd, ab, cd]
        };
        for (e, text) in seq {
            lattice
                .add_edge(
                    Span::new(0, 4).unwrap(),
                    EdgeCandidate::fused(text, [e]).unwrap(),
                )
                .unwrap();
        }
        lattice
            .edge(lattice.edges()[0].id())
            .unwrap()
            .candidate()
            .evidence()
            .to_vec()
    };
    let a = build(0);
    let b = build(1);
    assert_eq!(a, b);
    assert_eq!(a.len(), 2);
}

#[test]
fn same_text_different_span_stays_separate() {
    // 融合只发生在同一 (span, text);不同 span 是不同语义路径。
    let mut lattice = Lattice::new("abcdef".parse().unwrap());
    lattice
        .add_edge(
            Span::new(0, 2).unwrap(),
            cand("甲乙", CandidateKind::HotWord, 10),
        )
        .unwrap();
    lattice
        .add_edge(
            Span::new(0, 4).unwrap(),
            cand("甲乙丙丁", CandidateKind::HotWord, 20),
        )
        .unwrap();
    assert_eq!(lattice.edges().len(), 2);
}

#[test]
fn fused_candidate_flows_through_path_scoring() {
    use std::num::NonZeroUsize;
    use xhup_decoder::{BaselineScorer, RuntimeContext, rank_paths};

    let mut lattice = Lattice::new("abcd".parse().unwrap());
    lattice
        .add_edge(
            Span::new(0, 4).unwrap(),
            cand("甲乙", CandidateKind::HotWord, 100),
        )
        .unwrap();
    // 同一 (span, text) 的第二来源不应产生第二条路径。
    lattice
        .add_edge(
            Span::new(0, 4).unwrap(),
            cand("甲乙", CandidateKind::ExtendedWord, 80),
        )
        .unwrap();
    let paths = lattice.complete_paths(NonZeroUsize::new(8).unwrap());
    assert_eq!(paths.paths().len(), 1);
    let ranked = rank_paths(
        &BaselineScorer::default(),
        &RuntimeContext::new("", lattice.input().clone()),
        &lattice,
        paths.paths(),
    );
    // 融合后频率投影 = 最高来源频率(100),评分与单来源一致。
    assert_eq!(ranked.len(), 1);
}

#[test]
fn fused_constructor_rejects_empty_evidence_and_empty_text() {
    assert!(EdgeCandidate::fused("甲乙", std::iter::empty()).is_err());
    assert!(
        EdgeCandidate::fused("", [CandidateEvidence::new(CandidateKind::Character, 1)]).is_err()
    );
    // 合法多来源:
    let ok = EdgeCandidate::fused(
        "甲乙",
        [
            CandidateEvidence::new(CandidateKind::Character, 5),
            CandidateEvidence::new(CandidateKind::Character, 5),
            CandidateEvidence::new(CandidateKind::HotWord, 9),
        ],
    )
    .unwrap();
    assert_eq!(ok.evidence().len(), 2);
    assert_eq!(ok.evidence()[0].frequency(), 9);
}
