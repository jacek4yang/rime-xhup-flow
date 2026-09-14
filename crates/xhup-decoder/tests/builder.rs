use std::num::NonZeroUsize;

use xhup_decoder::{BuildStats, CandidateKind, LatticeBuilder, SourceCandidate};

fn fact(span: (usize, usize), text: &str, kind: CandidateKind, frequency: u64) -> SourceCandidate {
    SourceCandidate::new(span, text, kind, frequency)
}

#[test]
fn multi_source_facts_fuse_and_keep_single_edge_per_span_text() {
    // 研究生|命 与 研究|生命 两种分段,同文本不同来源:
    let builder = LatticeBuilder::new();
    let mut b = LatticeBuilder::new();
    b.push(fact((0, 4), "研究", CandidateKind::HotWord, 266_843));
    b.push(fact((4, 8), "生命", CandidateKind::HotWord, 80_039));
    b.push(fact((0, 6), "研究生", CandidateKind::ExtendedWord, 72_173));
    b.push(fact((6, 8), "命", CandidateKind::Character, 6_600));
    // 同 (span, text) 第二来源 → 融合:
    b.push(fact((0, 4), "研究", CandidateKind::ExtendedWord, 200_000));
    b.push(fact((0, 4), "研究", CandidateKind::HotWord, 266_843)); // 完全重复

    let built = b.build("yjjqugmk", NonZeroUsize::new(64).unwrap()).unwrap();
    let stats = built.stats();
    assert_eq!(
        *stats,
        BuildStats {
            facts: 6,
            edges: 4,
            fused_facts: 2
        }
    );
    assert_eq!(stats.fused_pairs(), 2);
    assert_eq!(built.lattice().edges().len(), 4);
    // 融合后的 "研究" 边 evidence: hot(266843) + extended(200000), 去重后 2 条:
    let edge = built
        .lattice()
        .edges()
        .iter()
        .find(|e| e.candidate().text() == "研究")
        .unwrap();
    assert_eq!(edge.candidate().evidence().len(), 2);
    assert_eq!(edge.candidate().frequency(), 266_843);
}

#[test]
fn fused_facts_do_not_duplicate_paths() {
    let mut b = LatticeBuilder::new();
    b.push(fact((0, 4), "甲乙", CandidateKind::HotWord, 10));
    b.push(fact((0, 4), "甲乙", CandidateKind::ExtendedWord, 5));
    b.push(fact((4, 8), "丙丁", CandidateKind::HotWord, 10));
    b.push(fact((4, 8), "丙丁", CandidateKind::ExtendedWord, 5));
    let built = b.build("abcdefgh", NonZeroUsize::new(8).unwrap()).unwrap();
    assert_eq!(built.paths().paths().len(), 1);
    assert!(!built.paths().truncated());
}

#[test]
fn empty_builder_and_bad_input_are_rejected() {
    let err = LatticeBuilder::new()
        .build("abcd", NonZeroUsize::new(4).unwrap())
        .unwrap_err();
    assert!(matches!(
        err,
        xhup_decoder::LatticeError::EmptyCandidateText
    ));
    let mut b = LatticeBuilder::new();
    b.push(fact((0, 2), "甲", CandidateKind::Character, 1));
    // 非法输入串(KeySequence 解析失败):
    let err = b.build("!!", NonZeroUsize::new(4).unwrap()).unwrap_err();
    assert!(matches!(
        err,
        xhup_decoder::LatticeError::InvalidSpan { .. }
    ));
}

#[test]
fn multi_evidence_push_accumulates_across_kinds() {
    let mut b = LatticeBuilder::new();
    b.push_multi(
        (0, 4),
        "甲乙".to_string(),
        &[
            xhup_decoder::CandidateEvidence::new(CandidateKind::HotWord, 100),
            xhup_decoder::CandidateEvidence::new(CandidateKind::ExtendedWord, 80),
            xhup_decoder::CandidateEvidence::new(CandidateKind::Character, 1),
        ],
    );
    let built = b.build("abcd", NonZeroUsize::new(4).unwrap()).unwrap();
    assert_eq!(built.stats().facts, 3);
    assert_eq!(built.stats().edges, 1);
    assert_eq!(built.stats().fused_facts, 2);
    let edge = built.lattice().edges()[0].candidate();
    assert_eq!(edge.evidence().len(), 3);
    assert_eq!(edge.evidence()[0].kind(), CandidateKind::HotWord);
    assert_eq!(edge.evidence()[2].kind(), CandidateKind::Character);
}
