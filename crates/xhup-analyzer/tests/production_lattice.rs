//! 桥接层测试:从生成器真实候选构建生产 lattice。

use std::num::NonZeroUsize;

use xhup_analyzer::production_lattice::{build_production_lattice, production_build_stats};
use xhup_decoder::CandidateKind;

const LIMIT: NonZeroUsize = NonZeroUsize::new(64).unwrap();

#[test]
fn production_lattice_contains_all_three_source_kinds() {
    // yj-jq-ug-mk = yan-jiu-sheng-ming 的双拼全码:
    // 研究(yj)+生命(ugmk)? 逐字双拼: yan=yj jiu=jq sheng=ug ming=mk
    // 4字词 "研究生命数据" 不一定在库中, 但 研究/生命/研究生 应存在。
    let built = build_production_lattice("yjjqugmk", LIMIT);
    let stats = built.stats();
    // 多来源事实流:单字原语 + hot 词 + 扩展词全部进入:
    assert!(stats.facts > 0);
    assert!(stats.edges > 0);
    // 融合发生:同 (span, text) 跨来源收拢(研究 既有 hot 也有 extended)。
    assert!(stats.edges < stats.facts);

    let mut kinds = std::collections::BTreeSet::new();
    for edge in built.lattice().edges() {
        kinds.insert(edge.candidate().kind());
    }
    assert!(kinds.contains(&CandidateKind::Character), "单字原语来源");
    assert!(kinds.contains(&CandidateKind::HotWord), "hot 词来源");
    assert!(kinds.contains(&CandidateKind::ExtendedWord), "扩展词来源");
}

#[test]
fn overlapping_spans_produce_multiple_segmentations() {
    // 研究生(6键) 与 研究(4键)+生命(4键) 同输入并存:
    let built = build_production_lattice("yjjqugmk", LIMIT);
    let paths = built.paths();
    assert!(!paths.paths().is_empty());
    // 至少两条完整路径(研究生|命 或 研究|生命 等):
    assert!(paths.paths().len() >= 2, "前缀闭合空间应允许多分段并存");
}

#[test]
fn hot_frequency_evidence_dominates_fused_rank_projection() {
    let built = build_production_lattice("yjjqugmk", LIMIT);
    // 融合边 (0,4) "研究":hot 分数 > extended 分数 → kind = HotWord 投影:
    let edge = built
        .lattice()
        .edges()
        .iter()
        .find(|e| e.span().start() == 0 && e.span().end() == 4 && e.candidate().text() == "研究")
        .expect("研究 必须以 (0,4) span 存在");
    assert_eq!(edge.candidate().kind(), CandidateKind::HotWord);
    assert!(edge.candidate().evidence().len() >= 2, "跨源证据已融合");
    assert!(edge.candidate().frequency() > 0);
}

#[test]
fn build_stats_are_deterministic_across_calls() {
    let a = production_build_stats("yjjqugmk", LIMIT);
    let b = production_build_stats("yjjqugmk", LIMIT);
    assert_eq!(a, b);
}
