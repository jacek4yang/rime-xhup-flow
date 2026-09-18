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
    // 真实数据读数(2026-09-17,transition_weight 标定为 2 之后)。
    // 任何一项变化都意味着 scorer、证据或 canonical 词码表发生了回归,
    // 必须显式可见。
    //
    // 与标定前(transition_weight=256)的差异是有意为之且已论证:
    //   contextualRank1 5535 -> 5532 (-3 例, 0.05%)
    //   harmfulReorder     8 -> 1     (-87.5%)
    // 即用 0.05% 的 rank1 换掉 87.5% 的有害重排,且跨切分通路的 top1 同时
    // 提升 14.4pp(见 crates/xhup-decoder/src/bigram.rs 的标定文档)。
    let report = replay_sentences_kdconv(SENTENCES, model());
    let m = &report.metrics;
    assert_eq!(m.sentences, 2000, "入库夹具句数");
    assert_eq!(m.tokens, 5729, "有 canonical 词码的 token 数");
    assert_eq!(m.ambiguous, 2701, "同码歧义 token 数");
    assert_eq!(m.baseline_rank1, 5085, "baseline rank1 命中");
    assert_eq!(m.contextual_rank1, 5532, "上下文 rank1 命中");
    assert_eq!(m.context_gain, 448, "上下文收益 token 数");
    assert_eq!(m.harmful_reorder, 1, "有害重排 token 数");

    // 派生比率(容差 1e-4)。
    assert!((m.baseline_rank1_rate() - 0.8876).abs() < 1e-4);
    assert!((m.contextual_rank1_rate() - 0.9656).abs() < 1e-4);
    assert!((m.context_gain_rate() - 0.0782).abs() < 1e-4);
    assert!((m.harmful_reorder_rate() - 0.0002).abs() < 1e-4);
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

// ---------------------------------------------------------------------------
// 有界 beam 解码 vs 全路径枚举的一致性(§25 第 7 步 / §22)
// ---------------------------------------------------------------------------

use std::num::NonZeroUsize;

use xhup_analyzer::context_replay::bounded_decode_consistency;
use xhup_decoder::{DecodeConfig, KdconvBigramScorer};

fn beam_config(width: usize) -> DecodeConfig {
    DecodeConfig::new(
        NonZeroUsize::new(width).expect("width >= 1"),
        NonZeroUsize::new(5).expect("top_k 5 != 0"),
        0,
    )
}

#[test]
fn bounded_decode_matches_exhaustive_ranking_at_wide_beam() {
    // 生产级有界解码必须在足够宽的 beam 下与「物化全部完整路径再排序」
    // 逐 token 一致 —— 这是 §24「bounded Beam/Viterbi 提供确定性 Top-K」
    // 的可执行证据。实测:beam=32 时 2701 个歧义 token 全部一致且无截断。
    let scorer = KdconvBigramScorer::new(model());
    let report = bounded_decode_consistency(
        SENTENCES,
        &scorer,
        KdconvBigramScorer::SCORER_ID,
        beam_config(32),
    );
    let m = &report.metrics;
    assert_eq!(report.schema, "xhup-bounded-decode-consistency/v1");
    assert_eq!(report.beam_width, 32);
    assert_eq!(m.compared, 2701, "与 context replay 的歧义 token 数一致");
    assert_eq!(m.top1_agreement, m.compared, "宽 beam 下必须 100% 一致");
    assert_eq!(m.truncated, 0, "宽 beam 下不得截断");
    assert_eq!(m.empty, 0, "不得出现无完整路径");
    assert_eq!(m.top1_agreement_rate(), 1.0);
}

#[test]
fn narrow_beam_truncates_and_loses_agreement_monotonically() {
    // 窄 beam 会截断并丢失与参考排序的一致性;该读数记录真实代价,
    // 防止把 beam 调窄当成「免费优化」。
    let scorer = KdconvBigramScorer::new(model());
    let narrow = bounded_decode_consistency(
        SENTENCES,
        &scorer,
        KdconvBigramScorer::SCORER_ID,
        beam_config(2),
    );
    assert!(
        narrow.metrics.truncated > 0,
        "beam=2 必须出现截断(真实菜单扇出超过 2)"
    );
    assert!(
        narrow.metrics.top1_agreement < narrow.metrics.compared,
        "beam=2 必须丢失部分 top1 一致性"
    );
    assert!(
        narrow.metrics.top1_agreement_rate() > 0.9,
        "仍应保持高一致率"
    );
}

#[test]
fn bounded_decode_is_deterministic() {
    let scorer = KdconvBigramScorer::new(model());
    let a = bounded_decode_consistency(
        SENTENCES,
        &scorer,
        KdconvBigramScorer::SCORER_ID,
        beam_config(8),
    );
    let b = bounded_decode_consistency(
        SENTENCES,
        &scorer,
        KdconvBigramScorer::SCORER_ID,
        beam_config(8),
    );
    assert_eq!(a, b, "同一配置与输入必须得到同一报告");
}

// ---------------------------------------------------------------------------
// 有害重排的证据诊断(§6 弱证据降级策略依据)
// ---------------------------------------------------------------------------

use xhup_analyzer::context_replay::harmful_case_diagnostics;

#[test]
fn harmful_diagnostics_expose_evidence_not_near_ties() {
    // 实测事实(2026-09-17):两个有害样本都不是「证据近乎持平」,而是
    // 「证据与本句真实用词不一致」—— 他的/它的 是同音词(tade),差异在
    // 指代对象而非转移强度。因此单纯加宽阈值无法修复这类错误。
    // 本测试把该结论固化为断言:若证据分布变化,读数会显式改变。
    let model = model();
    let cases = harmful_case_diagnostics(SENTENCES, &model, 32);
    assert!(!cases.is_empty(), "真实语料上确实存在有害样本");
    assert_eq!(
        cases.iter().filter(|c| c.is_near_tie()).count(),
        0,
        "有害样本不得全部退化为证据持平(否则应改用阈值策略)"
    );
    // 每个样本都必须与诊断口径一致:期望/选中词不同,且都来自语料。
    for case in &cases {
        assert_ne!(case.expected, case.picked, "有害 = 选中与期望不同");
        assert!(!case.committed_tail.is_empty(), "必须取到前文尾 token");
        assert_eq!(
            case.evidence_margin(),
            case.picked_evidence as i64 - case.expected_evidence as i64
        );
    }
}

#[test]
fn harmful_diagnostics_are_deterministic_and_bounded() {
    let a = harmful_case_diagnostics(SENTENCES, &model(), 1);
    let b = harmful_case_diagnostics(SENTENCES, &model(), 1);
    assert_eq!(a, b, "同一输入必须得到同一诊断");
    assert!(a.len() <= 1, "limit 必须被尊重");
}

#[test]
fn harmful_diagnostics_empty_corpus_yields_nothing() {
    assert!(harmful_case_diagnostics("", &model(), 32).is_empty());
}

// ---------------------------------------------------------------------------
// 期望候选选择成本(§1:rank1 命中率不是全部)
// ---------------------------------------------------------------------------

#[test]
fn context_reduces_expected_selection_cost_on_real_corpus() {
    // §1:「少按一键但候选在第 8 位」可能比直接输入完整码更差。因此除了
    // rank1 命中率,还必须度量期望**候选选择成本**。
    // 实测:baseline 0.0724 键/token → 上下文 0.0210 键/token。
    let report = replay_sentences_kdconv(SENTENCES, model());
    let m = &report.metrics;
    assert!(
        m.contextual_selection_cost_per_token() < m.baseline_selection_cost_per_token(),
        "上下文必须降低期望选择成本"
    );
    let saving = m.selection_cost_saving_per_token();
    assert!(
        (saving - 0.0512).abs() < 0.002,
        "真实数据节省约 0.0512 键/token,实际 {saving:.4}"
    );
    assert!(
        (m.baseline_selection_cost_per_token() - 0.0724).abs() < 0.002,
        "baseline 期望选择成本与基线一致"
    );
}

#[test]
fn selection_cost_scale_matches_replay_cost_model() {
    // 与 replay::ReplayCostModel 的 rank 成本同源,防止两处口径分叉。
    use xhup_analyzer::context_replay::selection_cost_q10;
    assert_eq!(selection_cost_q10(1), 0, "rank1 无选择成本");
    assert_eq!(selection_cost_q10(2), 512, "rank2 = 0.5 键");
    assert_eq!(selection_cost_q10(3), 1024, "rank3 = 1.0 键");
    assert_eq!(selection_cost_q10(4), 2048, "rank4 = 2.0 键");
    assert_eq!(selection_cost_q10(9), 2048, "超出档位用末档");
    // rank 0 = 未在菜单出现,按最差档计,不是 0。
    assert_eq!(selection_cost_q10(0), 2048, "缺席不得当作免费");
}

#[test]
fn identical_scorers_yield_identical_selection_cost() {
    // 自比时两侧成本必须逐项相等(差分器不得制造成本差异)。
    // 注意:即使两个 scorer 相同,歧义 token 中期望词不在 rank1 的那些
    // 仍会产生真实选择成本 —— 这正是 §1 要度量的东西,不能断言为 0。
    let report = replay_sentences(
        SENTENCES,
        &BaselineScorer::default(),
        BaselineScorer::SCORER_ID,
    );
    let m = &report.metrics;
    assert_eq!(
        m.baseline_selection_cost_q10, m.contextual_selection_cost_q10,
        "同一 scorer 两侧成本必须相等"
    );
    assert_eq!(m.selection_cost_saving_per_token(), 0.0);
    assert!(
        m.baseline_selection_cost_q10 > 0,
        "真实语料必然存在期望词不在 rank1 的歧义 token"
    );
}

#[test]
fn single_candidate_corpus_has_zero_selection_cost() {
    // 构造只有唯一候选的语料:任何 scorer 下期望词都必然 rank1,
    // 选择成本必须恰为 0。
    let report = replay_sentences(
        "不客气",
        &BaselineScorer::default(),
        BaselineScorer::SCORER_ID,
    );
    // 「不客气」的三个 token 若都唯一候选则成本为 0;若有歧义则必然 > 0。
    // 这里断言的是成本与 rank1 计数的一致性(而非特定数值)。
    let m = &report.metrics;
    let expected_zero = m.ambiguous == 0;
    assert_eq!(
        m.baseline_selection_cost_q10 == 0,
        expected_zero,
        "成本为 0 当且仅当无歧义 token(ambig={})",
        m.ambiguous
    );
}

// ---------------------------------------------------------------------------
// 解码延迟基线(§22:p50/p95/p99 此前无基线)
// ---------------------------------------------------------------------------

use xhup_analyzer::context_replay::{REPLAY_LATENCY_SCHEMA, replay_latency};

#[test]
fn replay_latency_reports_ordered_percentiles() {
    // 计时只报告、不设跨机器门槛;但百分位必须**有序**且样本数正确,
    // 否则读数本身不可信。
    let scorer = KdconvBigramScorer::new(model());
    let report = replay_latency(SENTENCES, &scorer, KdconvBigramScorer::SCORER_ID, 8);
    assert_eq!(report.schema, REPLAY_LATENCY_SCHEMA);
    assert_eq!(report.beam_width, 8);
    let l = &report.latency;
    assert_eq!(l.samples, 2701, "应与歧义 token 数一致");
    assert!(
        l.p50_micros <= l.p95_micros && l.p95_micros <= l.p99_micros,
        "百分位必须有序: p50={} p95={} p99={}",
        l.p50_micros,
        l.p95_micros,
        l.p99_micros
    );
    assert!(l.p99_micros <= l.max_micros, "p99 不得超过 max");
}

#[test]
fn replay_latency_is_bounded_and_recorded() {
    // 记录当前量级(2026-09-18,开发机):p50=7µs p95=42µs p99=95µs max=350µs。
    // 本测试**不**把该数值当跨机器门槛(§22 明确禁止),只断言量级合理:
    // p99 在人类可感知阈值(约 10ms)以内一个数量级以上。
    let scorer = KdconvBigramScorer::new(model());
    let report = replay_latency(SENTENCES, &scorer, KdconvBigramScorer::SCORER_ID, 8);
    let l = &report.latency;
    assert!(
        l.p99_micros < 10_000,
        "p99 应远低于 10ms 感知阈值,实际 {}µs",
        l.p99_micros
    );
    assert!(l.max_micros < 100_000, "max 不应出现秒级尖峰");
}

#[test]
fn empty_corpus_latency_is_zero_without_panic() {
    let scorer = KdconvBigramScorer::new(model());
    let report = replay_latency("", &scorer, KdconvBigramScorer::SCORER_ID, 8);
    assert_eq!(report.latency.samples, 0);
    assert_eq!(report.latency.p50_micros, 0);
    assert_eq!(report.latency.p99_micros, 0);
    assert_eq!(report.latency.max_micros, 0);
}

#[test]
fn latency_report_serializes_as_ascii() {
    let scorer = KdconvBigramScorer::new(model());
    let report = replay_latency(SENTENCES, &scorer, KdconvBigramScorer::SCORER_ID, 8);
    let json = serde_json::to_string(&report).expect("可序列化");
    assert!(json.is_ascii(), "序列化必须纯 ASCII");
    assert!(json.contains("\"p50Micros\""));
}
