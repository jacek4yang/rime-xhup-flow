//! Adversarial tiny-graph oracles, not production IME qualification.
use std::num::NonZeroUsize;
use xhup_decoder::{
    BaselineScorer, CandidateKind, DecodeConfig, DeterministicScorer, EdgeCandidate,
    FallbackReason, Lattice, LatticePath, RuntimeContext, Score, Span, decode_beam,
    decode_beam_adaptive, rank_paths,
};

fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn graph() -> Lattice {
    let mut graph = Lattice::new("abcd".parse().unwrap());
    for (start, end, text, frequency) in [
        (0, 2, "甲", 1_000_000),
        (0, 2, "乙", 1),
        (2, 4, "丙", 100),
        (2, 4, "丁", 1),
        (0, 4, "甲丙", 200),
    ] {
        graph
            .add_edge(
                Span::new(start, end).unwrap(),
                EdgeCandidate::new(text, CandidateKind::HotWord, frequency).unwrap(),
            )
            .unwrap();
    }
    graph
}
struct Objective {
    penalty: i64,
    bonus: i64,
    delayed: bool,
}
impl DeterministicScorer for Objective {
    type Breakdown = Score;
    fn score(
        &self,
        context: &RuntimeContext,
        graph: &Lattice,
        path: &LatticePath,
    ) -> (Score, Score) {
        let base = BaselineScorer::new(self.penalty)
            .score(context, graph, path)
            .0;
        let text: String = path
            .edge_ids()
            .iter()
            .map(|&id| graph.edge(id).unwrap().candidate().text())
            .collect();
        let favored = if self.delayed {
            text == "乙丁"
        } else {
            text.starts_with('乙')
        };
        let score = base + if favored { self.bonus } else { 0 };
        (score, score)
    }
}

#[test]
fn prefix_pruning_uses_the_callers_contextual_objective() {
    let graph = graph();
    let context = RuntimeContext::new("独立合成上下文", graph.input().clone());
    let scorer = Objective {
        penalty: 4096,
        bonus: 100_000,
        delayed: false,
    };
    let all = graph.complete_paths(nz(64));
    let exact = rank_paths(&scorer, &context, &graph, all.paths());
    assert!(exact[0].text().starts_with('乙'));
    let result = decode_beam(
        &graph,
        &context,
        &scorer,
        DecodeConfig::new(nz(1), nz(1), 0),
    );
    assert_eq!(result.ranked()[0].path(), exact[0].path());
    assert!(result.truncated()); // Matching top1 does not certify exhaustive search.
}

#[test]
fn tiny_graph_scorer_sweep_matches_exhaustive_when_untruncated() {
    let graph = graph();
    let context = RuntimeContext::new("", graph.input().clone());
    let all = graph.complete_paths(nz(64));
    assert!(!all.truncated());
    for penalty in [-10_000, 0, 4096, 100_000] {
        for bonus in [0, 10_000, 100_000] {
            for delayed in [false, true] {
                let scorer = Objective {
                    penalty,
                    bonus,
                    delayed,
                };
                let exact = rank_paths(&scorer, &context, &graph, all.paths());
                let (result, outcome) = decode_beam_adaptive(
                    &graph,
                    &context,
                    &scorer,
                    DecodeConfig::new(nz(1), nz(64), 0),
                    nz(64),
                );
                assert!(!outcome.truncated_at_max);
                assert!(
                    result.ranked() == exact,
                    "caller objective must match exhaustive oracle"
                );
            }
        }
    }
}

#[test]
fn future_reward_is_not_misrepresented_as_guaranteed_by_finite_beam() {
    let graph = graph();
    let context = RuntimeContext::new("", graph.input().clone());
    let scorer = Objective {
        penalty: 4096,
        bonus: 1_000_000,
        delayed: true,
    };
    let all = graph.complete_paths(nz(64));
    let exact = rank_paths(&scorer, &context, &graph, all.paths());
    assert_eq!(exact[0].text(), "乙丁");
    let result = decode_beam(
        &graph,
        &context,
        &scorer,
        DecodeConfig::new(nz(1), nz(1), 0),
    );
    assert_ne!(result.ranked()[0].text(), exact[0].text());
    assert_eq!(result.fallback(), Some(FallbackReason::BeamTruncated));
}
