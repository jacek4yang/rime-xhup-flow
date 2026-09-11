use std::num::NonZeroUsize;

use xhup_decoder::{
    BaselineScorer, CandidateKind, EdgeCandidate, Lattice, RuntimeContext, Span, rank_paths,
};

fn edge(text: &str, frequency: u64) -> EdgeCandidate {
    EdgeCandidate::new(text, CandidateKind::HotWord, frequency).unwrap()
}

#[test]
fn multiple_segmentations_coexist_for_the_same_raw_input() {
    // yj-jq-ug-mk = yan-jiu-sheng-ming。
    let input = "yjjqugmk".parse().unwrap();
    let mut lattice = Lattice::new(input);
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

    let paths = lattice.complete_paths(NonZeroUsize::new(8).unwrap());
    assert!(!paths.truncated());
    assert_eq!(paths.paths().len(), 2);

    let context = RuntimeContext::new("这个课题关注", lattice.input().clone());
    let ranked = rank_paths(
        &BaselineScorer::default(),
        &context,
        &lattice,
        paths.paths(),
    );
    let segmentations: Vec<_> = ranked
        .iter()
        .map(|path| path.segments().join("|"))
        .collect();
    assert_eq!(segmentations, ["研究|生命", "研究生|命"]);
    assert!(ranked[0].score() > ranked[1].score());
}

#[test]
fn path_materialization_is_explicitly_bounded() {
    let mut lattice = Lattice::new("abcd".parse().unwrap());
    for (start, end, text) in [(0, 2, "甲"), (0, 4, "甲乙"), (2, 4, "乙")] {
        lattice
            .add_edge(Span::new(start, end).unwrap(), edge(text, 1))
            .unwrap();
    }
    let paths = lattice.complete_paths(NonZeroUsize::new(1).unwrap());
    assert_eq!(paths.paths().len(), 1);
    assert!(paths.truncated());
}

#[test]
fn baseline_ranking_is_independent_of_left_context_and_deterministic() {
    let mut lattice = Lattice::new("abcd".parse().unwrap());
    lattice
        .add_edge(Span::new(0, 4).unwrap(), edge("甲乙", 10))
        .unwrap();
    lattice
        .add_edge(Span::new(0, 2).unwrap(), edge("甲", 100))
        .unwrap();
    lattice
        .add_edge(Span::new(2, 4).unwrap(), edge("乙", 100))
        .unwrap();
    let paths = lattice.complete_paths(NonZeroUsize::new(8).unwrap());
    let scorer = BaselineScorer::default();
    let a = rank_paths(
        &scorer,
        &RuntimeContext::new("上下文甲", lattice.input().clone()),
        &lattice,
        paths.paths(),
    );
    let b = rank_paths(
        &scorer,
        &RuntimeContext::new("上下文乙", lattice.input().clone()),
        &lattice,
        paths.paths(),
    );
    assert!(
        a == b,
        "foundation baseline 尚不消费上下文，作为后续 A/B 对照"
    );
}
