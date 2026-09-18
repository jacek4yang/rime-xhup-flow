//! 跨切分(码级)评测通路的契约测试(Issue #83 §20/§24)。
//!
//! 断言的是**度量口径与排除语义**,不是特定正确率数值:排除分类必须显式,
//! 分母只含有效样本,且「文本正确」与「逐 span 严格」必须分别报告。

use xhup_analyzer::cross_segmentation::{
    CROSS_SEGMENTATION_SCHEMA, MAX_INPUT_KEYS, evaluate_cross_segmentation_baseline,
};

const SENTENCES: &str = include_str!("../../../data/corpus/replay_fixture.txt");

#[test]
fn empty_corpus_yields_zero_metrics_without_panic() {
    let report = evaluate_cross_segmentation_baseline("", 5);
    let m = &report.metrics;
    assert_eq!(m.sentences, 0);
    assert_eq!(m.evaluated, 0);
    assert_eq!(m.top1_text_correct, 0);
    // 空分母的比率必须是 0,不是 NaN。
    assert_eq!(m.top1_text_rate(), 0.0);
    assert_eq!(m.top_k_rate(), 0.0);
    assert_eq!(m.exclusion_rate(), 0.0);
}

#[test]
fn report_schema_and_config_are_versioned_and_explicit() {
    let report = evaluate_cross_segmentation_baseline("不客气", 5);
    assert_eq!(report.schema, CROSS_SEGMENTATION_SCHEMA);
    assert_eq!(report.scorer, "word-frequency-segmentation/v1");
    assert_eq!(report.top_k, 5);
    assert!(report.beam_floor >= 1);
    assert!(
        report.beam_ceiling >= report.beam_floor,
        "上限不得低于起始宽度"
    );
    let json = serde_json::to_string(&report).expect("报告可序列化");
    assert!(json.is_ascii(), "序列化必须是纯 ASCII:{json}");
    assert!(json.contains("\"top1TextCorrect\""));
    assert!(json.contains("\"truncatedAtMax\""));
}

#[test]
fn exclusion_classes_are_explicit_and_mutually_accounted() {
    // 排除必须分类计数:不可达 ≠ 排序错误。分母只含有效样本。
    let report = cached(false);
    let m = &report.metrics;
    assert_eq!(m.sentences, 2000);
    let excluded = m.skipped_no_word_code + m.skipped_input_bounds + m.skipped_expected_path_absent;
    assert_eq!(
        m.evaluated + excluded,
        m.sentences,
        "有效 + 排除必须恰好等于总句数"
    );
    assert!(
        m.skipped_no_word_code > 0,
        "真实语料必然有无词码 token 的句子"
    );
    assert!(
        (m.exclusion_rate() - excluded as f64 / 2000.0).abs() < 1e-9,
        "排除率口径必须与分子分母一致"
    );
}

#[test]
fn top1_counts_never_exceed_evaluated() {
    let report = cached(false);
    let m = &report.metrics;
    assert!(m.top1_text_correct <= m.evaluated);
    assert!(m.top1_span_exact <= m.evaluated);
    assert!(m.in_top_k <= m.evaluated);
    assert!(m.truncated_at_max <= m.evaluated);
    // top-k 命中必然不少于 top1 命中。
    assert!(m.in_top_k >= m.top1_text_correct);
}

#[test]
fn text_and_span_metrics_are_reported_separately() {
    // 同一文本可由多种合法切分产生(如 ["好的"] vs ["好","的"]),
    // 因此按 span 严格比较会把正确结果判为错误 —— 两个口径必须分开报告,
    // 且文本口径(产品关注)不低于严格口径。
    let report = cached(false);
    let m = &report.metrics;
    assert!(
        m.top1_text_correct >= m.top1_span_exact,
        "文本正确必然涵盖逐 span 精确正确"
    );
}

#[test]
fn evaluation_is_deterministic() {
    let a = evaluate_cross_segmentation_baseline(SENTENCES, 5);
    let b = evaluate_cross_segmentation_baseline(SENTENCES, 5);
    assert_eq!(a, b, "同输入必须得到同一报告");
}

#[test]
fn overlong_input_is_excluded_by_some_explicit_class() {
    // 超长输入必须被**显式排除**(计入某个排除分类),不得静默进入分母。
    let long_sentence = "这个景点的地址在哪呢请问一下好不好呀谢谢你了"; // 远超上限
    let report = evaluate_cross_segmentation_baseline(long_sentence, 5);
    let m = &report.metrics;
    assert_eq!(m.sentences, 1);
    assert_eq!(m.evaluated, 0, "超长输入不得进入有效样本");
    let excluded = m.skipped_no_word_code + m.skipped_input_bounds + m.skipped_expected_path_absent;
    assert_eq!(excluded, 1, "必须恰好被一个排除分类捕获:{m:?}");
    const { assert!(MAX_INPUT_KEYS >= 4) };
}

#[test]
fn no_word_code_sentence_is_excluded_not_counted_as_error() {
    // 纯 ASCII/无词码输入必须走排除,不得进入分母。
    let report = evaluate_cross_segmentation_baseline("xyzzy", 5);
    let m = &report.metrics;
    assert_eq!(m.evaluated, 0);
    assert_eq!(m.top1_text_correct, 0);
    assert!(m.skipped_no_word_code + m.skipped_input_bounds >= 1);
}

// ---------------------------------------------------------------------------
// committed context 窗口模式
// ---------------------------------------------------------------------------

use xhup_analyzer::cross_segmentation::evaluate_cross_segmentation;
use xhup_decoder::{BaselineScorer, BigramModel, KdconvBigramScorer};

const BIGRAM_TSV: &str = include_str!("../../../data/corpus/kdconv_bigram.tsv");

#[test]
fn committed_context_mode_is_explicit_in_report_and_counted() {
    let report = cached(true);
    assert!(report.with_committed_context, "报告必须显式标注模式");
    let m = &report.metrics;
    // 窗口数 > 0 且已计入排除账;有效 + 排除仍须恰好等于总句数。
    assert!(m.contextual_windows > 0, "多 token 句子应切出窗口");
    let excluded = m.skipped_no_word_code + m.skipped_input_bounds + m.skipped_expected_path_absent;
    assert_eq!(
        m.evaluated + excluded,
        m.sentences,
        "两条路径(单句/窗口)的计数之和仍须恰好覆盖总句数"
    );
}

#[test]
fn committed_context_does_not_break_invariants() {
    let report = cached(true);
    let m = &report.metrics;
    assert!(m.top1_text_correct <= m.evaluated);
    assert!(m.top1_span_exact <= m.evaluated);
    assert!(m.in_top_k <= m.evaluated);
    assert!(m.in_top_k >= m.top1_text_correct);
}

#[test]
fn committed_context_mode_is_deterministic() {
    let a = cached(true);
    let b = cached(true);
    assert_eq!(a, b);
}

#[test]
fn committed_context_improves_bigram_top_k_on_real_corpus() {
    // 实测事实(2026-09-17):committed context 让 bigram scorer 的 top-k 命中
    // 从 0.4539 提升到 0.6640 —— 这是「上下文真实改善**跨切分**选择」的第一个
    // 可执行证据,与本仓库此前的同码词消歧通路(#108)是不同总体。
    //
    // 同时记录一个**负面**读数:该 scorer 的 top1 仍低于 baseline
    // (0.2628 vs 0.4130),说明 transition_weight 相对词频权重对跨切分任务
    // 偏高。本测试只锁定「上下文有用」这一方向性事实,不锁定具体数值。
    let without = cached_bigram(false);
    let with = cached_bigram(true);
    assert!(with.metrics.evaluated > 0, "必须有有效样本");
    assert!(
        with.metrics.top_k_rate() > without.metrics.top_k_rate(),
        "committed context 应提升 top-k(实测 {:.4} -> {:.4})",
        without.metrics.top_k_rate(),
        with.metrics.top_k_rate()
    );
}

/// 全语料评测很贵(每句都要构造生产 lattice),按模式缓存一次。
fn cached(with_ctx: bool) -> &'static xhup_analyzer::cross_segmentation::CrossSegmentationReport {
    use std::sync::OnceLock;
    static NO_CTX: OnceLock<xhup_analyzer::cross_segmentation::CrossSegmentationReport> =
        OnceLock::new();
    static WITH_CTX: OnceLock<xhup_analyzer::cross_segmentation::CrossSegmentationReport> =
        OnceLock::new();
    if with_ctx {
        WITH_CTX.get_or_init(|| {
            evaluate_cross_segmentation(
                SENTENCES,
                &BaselineScorer::default(),
                BaselineScorer::SCORER_ID,
                5,
                true,
            )
        })
    } else {
        NO_CTX.get_or_init(|| {
            evaluate_cross_segmentation(
                SENTENCES,
                &BaselineScorer::default(),
                BaselineScorer::SCORER_ID,
                5,
                false,
            )
        })
    }
}

fn cached_bigram(
    with_ctx: bool,
) -> &'static xhup_analyzer::cross_segmentation::CrossSegmentationReport {
    use std::sync::OnceLock;
    static NO_CTX: OnceLock<xhup_analyzer::cross_segmentation::CrossSegmentationReport> =
        OnceLock::new();
    static WITH_CTX: OnceLock<xhup_analyzer::cross_segmentation::CrossSegmentationReport> =
        OnceLock::new();
    if with_ctx {
        WITH_CTX.get_or_init(|| {
            let model = BigramModel::from_tsv(BIGRAM_TSV).expect("bigram TSV 可解析");
            evaluate_cross_segmentation(
                SENTENCES,
                &KdconvBigramScorer::new(model),
                KdconvBigramScorer::SCORER_ID,
                5,
                true,
            )
        })
    } else {
        NO_CTX.get_or_init(|| {
            let model = BigramModel::from_tsv(BIGRAM_TSV).expect("bigram TSV 可解析");
            evaluate_cross_segmentation(
                SENTENCES,
                &KdconvBigramScorer::new(model),
                KdconvBigramScorer::SCORER_ID,
                5,
                false,
            )
        })
    }
}
