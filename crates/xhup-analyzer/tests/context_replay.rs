//! 真实语料 committed-context 回放的回归测试(Issue #83 §20/§24)。
//!
//! 断言的是**真实数据上的读数值**,不是具体词的排序:这些数字是上下文解码
//! 收益的唯一可执行证据,任何 scorer/证据/码表回归都必须让它们显式变化。

use xhup_analyzer::context_replay::{replay_sentences, replay_sentences_kdconv};
use xhup_decoder::{BaselineScorer, BigramModel};

const SENTENCES: &str = include_str!("../../../data/corpus/replay_fixture.txt");
const BIGRAM_TSV: &str = include_str!("../../../data/corpus/kdconv_bigram.tsv");

fn model() -> BigramModel {
    BigramModel::from_tsv(BIGRAM_TSV).expect("真实 bigram TSV 必须可解析")
}

#[test]
fn real_corpus_context_replay_matches_committed_numbers() {
    // 真实数据读数(2026-09-17)。任何一项变化都意味着 scorer、证据或
    // canonical 词码表发生了回归,必须显式可见。
    let report = replay_sentences_kdconv(SENTENCES, model());
    let m = &report.metrics;
    assert_eq!(m.sentences, 2000, "入库夹具句数");
    assert_eq!(m.tokens, 5729, "有 canonical 词码的 token 数");
    assert_eq!(m.ambiguous, 2701, "同码歧义 token 数");
    assert_eq!(m.baseline_rank1, 5085, "baseline rank1 命中");
    assert_eq!(m.contextual_rank1, 5535, "上下文 rank1 命中");
    assert_eq!(m.context_gain, 458, "上下文收益 token 数");
    assert_eq!(m.harmful_reorder, 8, "有害重排 token 数");

    // 派生比率(容差 1e-4)。
    assert!((m.baseline_rank1_rate() - 0.8876).abs() < 1e-4);
    assert!((m.contextual_rank1_rate() - 0.9661).abs() < 1e-4);
    assert!((m.context_gain_rate() - 0.0799).abs() < 1e-4);
    assert!((m.harmful_reorder_rate() - 0.0014).abs() < 1e-4);
}

#[test]
fn context_replay_is_deterministic() {
    let a = replay_sentences_kdconv(SENTENCES, model());
    let b = replay_sentences_kdconv(SENTENCES, model());
    assert_eq!(a, b, "同一输入必须得到同一报告(固定点评分 + 有序容器)");
}

#[test]
fn baseline_self_comparison_yields_no_gain_and_no_harm() {
    // 用 baseline 自身当「上下文 scorer」:差分器不得制造信号。
    // 这同时证明观测到的增益不是「换了个打分函数」的假象。
    let report = replay_sentences(
        SENTENCES,
        &BaselineScorer::default(),
        BaselineScorer::SCORER_ID,
    );
    let m = &report.metrics;
    assert_eq!(m.context_gain, 0, "同 scorer 不得有增益");
    assert_eq!(m.harmful_reorder, 0, "同 scorer 不得有退化");
    assert_eq!(m.baseline_rank1, m.contextual_rank1);
    assert_eq!(report.baseline_scorer, report.contextual_scorer);
}

#[test]
fn context_gain_is_positive_and_harm_is_bounded_on_real_corpus() {
    // 产品契约(§24):committed context 必须真实改善,且不得以牺牲已正确
    // 路径为代价。8 个 harmful 全部是同一处真实近义歧义(他的 227 vs
    // 它的 230),不是系统性退化;门槛固定在实测值的合理余量内。
    let report = replay_sentences_kdconv(SENTENCES, model());
    let m = &report.metrics;
    assert!(
        m.contextual_rank1_rate() > m.baseline_rank1_rate(),
        "上下文必须提升 rank1 命中率"
    );
    assert!(
        m.net_gain() > 0.05,
        "净收益必须显著为正:{:+.4}",
        m.net_gain()
    );
    assert!(
        m.harmful_reorder_rate() < 0.005,
        "有害重排率必须 < 0.5%: {:.4}",
        m.harmful_reorder_rate()
    );
    assert!(m.ambiguous > 0, "夹具必须包含真实同码歧义");
}

#[test]
fn empty_corpus_yields_zero_metrics_without_panic() {
    let report = replay_sentences_kdconv("", model());
    let m = &report.metrics;
    assert_eq!(m.sentences, 0);
    assert_eq!(m.tokens, 0);
    assert_eq!(m.context_gain, 0);
    assert_eq!(m.harmful_reorder, 0);
    // 空分母的比率必须是 0,不是 NaN。
    assert_eq!(m.baseline_rank1_rate(), 0.0);
    assert_eq!(m.context_gain_rate(), 0.0);
    assert_eq!(m.harmful_reorder_rate(), 0.0);
    assert!(m.net_gain().is_finite());
}

#[test]
fn report_schema_is_versioned_and_serializable() {
    let report = replay_sentences_kdconv("这个景点的地址在哪呢？", model());
    assert_eq!(report.schema, "xhup-context-replay/v1");
    assert_eq!(report.baseline_scorer, "word-frequency-segmentation/v1");
    assert_eq!(report.contextual_scorer, "kdconv-bigram/v1");
    let json = serde_json::to_string(&report).expect("报告必须可序列化");
    assert!(json.is_ascii(), "序列化输出必须是纯 ASCII:{json}");
    assert!(json.contains("\"contextGain\""));
    assert!(json.contains("\"harmfulReorder\""));
}
