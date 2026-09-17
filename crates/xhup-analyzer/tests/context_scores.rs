//! 跨 scorer 差分(context gain / harmful reorder rate)的真实数据测试。
//!
//! 这些测试锁定的是**度量口径与语义契约**,不是具体词的具体排序:
//!
//! - 相同 scorer 自比必须零增益零退化(差分不制造假信号);
//! - 已知有真实转移证据的合成 lattice 上,上下文必须改变首选路径(增益为正);
//! - 真实 canonical + 真实 KDConv 数据下的 measured baseline 必须与
//!   committed fixture 一致(防止 fixture 与生产数据静默漂移);
//! - harmful reorder 在真实数据上必须为 0(§6 弱证据平滑降级)。

use std::num::NonZeroUsize;

use xhup_analyzer::contextual_benchmark::{
    BenchmarkRunner, BenchmarkSuite, ScoreDelta, compare_runs,
};
use xhup_core::KeySequence;
use xhup_decoder::{
    BaselineScorer, BigramModel, CandidateKind, EdgeCandidate, KdconvBigramScorer, Lattice,
    RuntimeContext, Span, rank_paths,
};

const FIXTURE: &str = include_str!("../../../data/benchmarks/contextual-v1.json");
const BASELINE: &str = include_str!("../../../data/benchmarks/contextual-baseline-v1.json");
const BIGRAM_TSV: &str = include_str!("../../../data/corpus/kdconv_bigram.tsv");

fn load_bigram() -> BigramModel {
    BigramModel::from_tsv(BIGRAM_TSV).expect("真实 bigram TSV 必须可解析")
}

#[test]
fn identical_scorers_produce_zero_gain_and_zero_regression() {
    // 差分器不能在两侧相同时制造信号;这是所有后续读数的可信前提。
    let suite = BenchmarkSuite::from_json(FIXTURE).unwrap();
    let a = BenchmarkRunner::<BaselineScorer>::default().run(&suite);
    let b = BenchmarkRunner::<BaselineScorer>::default().run(&suite);
    let comparison = compare_runs(&a, &b);
    assert_eq!(comparison.improved, 0);
    assert_eq!(comparison.regressed, 0);
    assert_eq!(comparison.compared, suite.cases().len());
    assert_eq!(comparison.context_gain(), 0.0);
    assert_eq!(comparison.harmful_reorder_rate(), 0.0);
    assert_eq!(comparison.net_gain(), 0.0);
    assert!(
        comparison
            .deltas
            .values()
            .all(|d| *d == ScoreDelta::Unchanged)
    );
}

#[test]
fn committed_fixture_shows_no_context_gain_on_real_kdconv_evidence() {
    // 实测事实(2026-09-17):现有 4 例 fixture 的决胜转移
    // (研究→生命 / 研究生→命 / …)在 KDConv 中计数为 0,因此真实
    // bigram scorer 既不能改善也不能破坏 baseline。本测试把该事实
    // 固化为回归哨兵:一旦 fixture 或证据变化,读数会显式改变。
    let suite = BenchmarkSuite::from_json(FIXTURE).unwrap();
    let baseline_run = BenchmarkRunner::<BaselineScorer>::default().run(&suite);
    let contextual_run = BenchmarkRunner::new(KdconvBigramScorer::new(load_bigram())).run(&suite);
    let comparison = compare_runs(&baseline_run, &contextual_run);

    assert_eq!(comparison.compared, 4);
    assert_eq!(
        comparison.baseline_top1, 2,
        "baseline 命中 2 例(见 committed baseline)"
    );
    assert_eq!(comparison.context_gain(), 0.0, "现有 fixture 无上下文增益");
    assert_eq!(
        comparison.harmful_reorder_rate(),
        0.0,
        "上下文不得破坏已正确路径"
    );
    assert_eq!(contextual_run.report().scorer, "kdconv-bigram/v1");
    // baseline 报告仍必须与 committed baseline 精确一致(口径未漂移)。
    assert!(
        xhup_analyzer::contextual_benchmark::check_baseline_json(baseline_run.report(), BASELINE)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn real_transition_evidence_flips_ranking_toward_context_supported_path() {
    // 选取真实 canonical 码表与真实 KDConv 数据中证据强的一对词:
    //   「这个」(vege) + 「景点」(jd) → 2019 次转移
    //   「这个」(vege) + 「乐队」(yd) → 146 次转移
    // 两路右侧码均为 2 键,故两种切分总键数相同(baseline 分数只由词频
    // 决定,与切分无关),决策完全交给转移证据。该测试证明「committed
    // context 能真实改变首选路径」这条链路是通的;上面那个 fixture 测试
    // 则证明现有 4 例样本还没有这样的证据。
    let model = load_bigram();
    let (left, supported, unsupported) = ("这个", "景点", "乐队");
    assert!(
        model.transition_count(left, supported) > model.transition_count(left, unsupported),
        "选取的一对必须有真实证据差"
    );

    // 输入 6 键:head「这个」4 键 + 右侧 2 键(两种切分完全覆盖且总长一致)。
    let input = "vegejd".parse::<KeySequence>().unwrap();
    let mut lattice = Lattice::new(input.clone());
    // 右侧两词共享同一 span,词频刻意让 baseline 偏向无证据者。
    for (text, start, end, freq) in [
        ("这个", 0, 4, 500_000_u64),
        ("景点", 4, 6, 40_000),
        ("乐队", 4, 6, 60_000),
    ] {
        lattice
            .add_edge(
                Span::new(start, end).unwrap(),
                EdgeCandidate::new(text, CandidateKind::HotWord, freq).unwrap(),
            )
            .unwrap();
    }

    let context = RuntimeContext::new(left, input);
    let paths = lattice.complete_paths(NonZeroUsize::new(32).unwrap());
    let baseline = rank_paths(
        &BaselineScorer::default(),
        &context,
        &lattice,
        paths.paths(),
    );
    let paths = lattice.complete_paths(NonZeroUsize::new(32).unwrap());
    let contextual = rank_paths(
        &KdconvBigramScorer::new(model),
        &context,
        &lattice,
        paths.paths(),
    );

    // baseline 忽略上下文,必然选词频更高的「乐队」。
    assert_eq!(baseline[0].segments(), ["这个", "乐队"]);
    // 上下文 scorer 选有真实转移证据的「景点」。
    assert_eq!(
        contextual[0].segments(),
        ["这个", "景点"],
        "committed context 必须把首选改到有真实转移证据的路径"
    );
}

#[test]
fn harmful_reorder_is_zero_on_real_kdconv_evidence() {
    // §6:弱证据必须平滑降级。KDConv 覆盖稀疏(5308 个可二分歧义实例中
    // 仅 83 个有任何 in-path 证据),所以上下文不得系统性地破坏 baseline。
    let suite = BenchmarkSuite::from_json(FIXTURE).unwrap();
    let baseline_run = BenchmarkRunner::<BaselineScorer>::default().run(&suite);
    let contextual_run = BenchmarkRunner::new(KdconvBigramScorer::new(load_bigram())).run(&suite);
    let comparison = compare_runs(&baseline_run, &contextual_run);
    assert_eq!(
        comparison.harmful_cases(),
        0,
        "真实数据上不得出现有害重排:{}",
        comparison.render_ascii()
    );
    assert!(comparison.net_gain() >= 0.0);
}

#[test]
fn ascii_report_is_deterministic_and_ascii_only() {
    let suite = BenchmarkSuite::from_json(FIXTURE).unwrap();
    let baseline_run = BenchmarkRunner::<BaselineScorer>::default().run(&suite);
    let contextual_run = BenchmarkRunner::new(KdconvBigramScorer::new(load_bigram())).run(&suite);
    let a = compare_runs(&baseline_run, &contextual_run).render_ascii();
    let b = compare_runs(&baseline_run, &contextual_run).render_ascii();
    assert_eq!(a, b, "报告必须字节稳定");
    assert!(a.is_ascii(), "报告必须是纯 ASCII(§7 极简呈现准则):{a}");
    assert!(a.contains("harmful_reorder_rate"));
    assert!(a.contains("context_gain_rate"));
}
