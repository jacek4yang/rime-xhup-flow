//! `context-replay-bench`:真实语料 committed-context 回放基准(Issue #83 §20/§24)。
//!
//! 对真实句子语料逐 token 回放,比较 baseline(忽略上下文)与 KDConv bigram
//! scorer 的 rank1 命中与有害重排率。可选 `--baseline` 做回归门禁断言。
//!
//! 全离线:只读显式传入的文件,不联网、不写用户数据。

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::process::ExitCode;

use xhup_analyzer::context_replay::{
    BoundedDecodeReport, ContextReplayReport, ReplayLatencyReport, bounded_decode_consistency,
    harmful_case_diagnostics, replay_latency, replay_sentences_kdconv,
};
use xhup_decoder::{BigramModel, DecodeConfig, KdconvBigramScorer};

fn usage() -> ! {
    eprintln!(
        "用法: context-replay-bench --sentences <replay_fixture.txt> --bigram <kdconv_bigram.tsv>\n\
         \x20     [--json] [--baseline <context-replay-baseline.json>]\n\
         \x20     [--bounded] [--beam N]\n\
         对真实语料逐 token 回放,输出 committed-context 的 rank1 命中与\n\
         harmful reorder rate(§20)。baseline 只断言非计时指标。\n\
         --bounded   额外输出有界 beam 解码 vs 全路径枚举的 top1 一致性\n\
         --beam N    有界解码 beam 宽度(缺省 8)
         --harmful   额外输出有害重排样本的证据明细(§6 降级策略依据)
         --latency   额外输出解码延迟 p50/p95/p99(§22;仅报告不设门槛)"
    );
    std::process::exit(2);
}

fn main() -> ExitCode {
    let mut sentences: Option<PathBuf> = None;
    let mut bigram: Option<PathBuf> = None;
    let mut baseline: Option<PathBuf> = None;
    let mut json = false;
    let mut bounded = false;
    let mut harmful = false;
    let mut latency = false;
    let mut beam_width = 8usize;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--sentences" => {
                sentences = Some(PathBuf::from(args.next().unwrap_or_else(|| usage())))
            }
            "--bigram" => bigram = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            "--baseline" => baseline = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            "--json" => json = true,
            "--bounded" => bounded = true,
            "--harmful" => harmful = true,
            "--latency" => latency = true,
            "--beam" => {
                let value = args.next().unwrap_or_else(|| usage());
                beam_width = match value.parse() {
                    Ok(parsed) if parsed > 0 => parsed,
                    _ => {
                        eprintln!("--beam 必须 ≥ 1,实际 {value:?}");
                        return ExitCode::FAILURE;
                    }
                };
            }
            _ => usage(),
        }
    }
    let (Some(sentences), Some(bigram)) = (sentences, bigram) else {
        usage();
    };

    let sentences_text = match std::fs::read_to_string(&sentences) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("无法读取语料 {}: {error}", sentences.display());
            return ExitCode::FAILURE;
        }
    };
    let bigram_text = match std::fs::read_to_string(&bigram) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("无法读取 bigram {}: {error}", bigram.display());
            return ExitCode::FAILURE;
        }
    };
    let model = match BigramModel::from_tsv(&bigram_text) {
        Ok(model) => model,
        Err(error) => {
            eprintln!("bigram TSV 非法: {error}");
            return ExitCode::FAILURE;
        }
    };

    let report = replay_sentences_kdconv(&sentences_text, model.clone());
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("ContextReplayReport 必然可序列化")
        );
    } else {
        print!("{}", render_text(&report));
    }

    // 有界 beam 解码 vs 全路径枚举的一致性(§25 第 7 步)。
    if bounded {
        let scorer = KdconvBigramScorer::new(model.clone());
        let config = DecodeConfig::new(
            NonZeroUsize::new(beam_width).expect("--beam 必须 ≥ 1"),
            NonZeroUsize::new(5).expect("top_k 5 != 0"),
            0,
        );
        let bounded_report = bounded_decode_consistency(
            &sentences_text,
            &scorer,
            KdconvBigramScorer::SCORER_ID,
            config,
        );
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&bounded_report)
                    .expect("BoundedDecodeReport 必然可序列化")
            );
        } else {
            print!("{}", render_bounded_text(&bounded_report));
        }
    }

    // 有害重排的证据明细(§6)。只含语料词与聚合计数,不含用户数据。
    if harmful {
        let cases = harmful_case_diagnostics(&sentences_text, &model, 32);
        let near_ties = cases.iter().filter(|c| c.is_near_tie()).count();
        println!("harmful_samples: {}", cases.len());
        println!("harmful_near_ties: {near_ties}");
        for case in &cases {
            println!(
                "  tail={} expected={} ({} ) picked={} ({}) margin={}",
                case.committed_tail,
                case.expected,
                case.expected_evidence,
                case.picked,
                case.picked_evidence,
                case.evidence_margin()
            );
        }
    }

    // 解码延迟(§22)。仅报告,不设跨机器门槛。
    if latency {
        let scorer = KdconvBigramScorer::new(model.clone());
        let report = replay_latency(&sentences_text, &scorer, KdconvBigramScorer::SCORER_ID, 8);
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).expect("ReplayLatencyReport 可序列化")
            );
        } else {
            print!("{}", render_latency(&report));
        }
    }

    if let Some(path) = baseline {
        let expected_text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("无法读取 baseline {}: {error}", path.display());
                return ExitCode::FAILURE;
            }
        };
        match check_baseline(&report, &expected_text) {
            Ok(failures) if failures.is_empty() => {
                eprintln!("context replay baseline 断言通过");
            }
            Ok(failures) => {
                eprintln!("context replay baseline 断言失败:");
                for failure in failures {
                    eprintln!("  {failure}");
                }
                return ExitCode::FAILURE;
            }
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
        }
    }

    ExitCode::SUCCESS
}

/// 人类可读的确定性 ASCII 报告(无 emoji,§7)。
fn render_text(report: &ContextReplayReport) -> String {
    let m = &report.metrics;
    let mut out = String::new();
    out.push_str(&format!("schema:            {}\n", report.schema));
    out.push_str(&format!(
        "scorers:           {} -> {}\n",
        report.baseline_scorer, report.contextual_scorer
    ));
    out.push_str(&format!("sentences:         {}\n", m.sentences));
    out.push_str(&format!("tokens:            {}\n", m.tokens));
    out.push_str(&format!("ambiguous:         {}\n", m.ambiguous));
    out.push_str(&format!("baseline_rank1:    {}\n", m.baseline_rank1));
    out.push_str(&format!("contextual_rank1:  {}\n", m.contextual_rank1));
    out.push_str(&format!("context_gain:      {}\n", m.context_gain));
    out.push_str(&format!("harmful_reorder:   {}\n", m.harmful_reorder));
    out.push_str(&format!(
        "baseline_rank1_rate:   {:.4}\n",
        m.baseline_rank1_rate()
    ));
    out.push_str(&format!(
        "contextual_rank1_rate: {:.4}\n",
        m.contextual_rank1_rate()
    ));
    out.push_str(&format!(
        "context_gain_rate:     {:.4}\n",
        m.context_gain_rate()
    ));
    out.push_str(&format!(
        "harmful_reorder_rate:  {:.4}\n",
        m.harmful_reorder_rate()
    ));
    out.push_str(&format!("net_gain:              {:+.4}\n", m.net_gain()));
    // §1:rank1 命中率不是全部 —— 「少按一键但候选在第 8 位」可能更差。
    out.push_str(&format!(
        "baseline_sel_cost/tok:   {:.4}\n",
        m.baseline_selection_cost_per_token()
    ));
    out.push_str(&format!(
        "contextual_sel_cost/tok: {:.4}\n",
        m.contextual_selection_cost_per_token()
    ));
    out.push_str(&format!(
        "selection_saving/token:  {:+.4}\n",
        m.selection_cost_saving_per_token()
    ));
    out
}

/// 解码延迟的确定性 ASCII 报告(微秒)。
fn render_latency(report: &ReplayLatencyReport) -> String {
    let l = &report.latency;
    let mut out = String::new();
    out.push_str(&format!(
        "schema:              {}
",
        report.schema
    ));
    out.push_str(&format!(
        "scorer:              {}
",
        report.scorer
    ));
    out.push_str(&format!(
        "beam_width:          {}
",
        report.beam_width
    ));
    out.push_str(&format!(
        "samples:             {}
",
        l.samples
    ));
    out.push_str(&format!(
        "latency_p50_micros:  {}
",
        l.p50_micros
    ));
    out.push_str(&format!(
        "latency_p95_micros:  {}
",
        l.p95_micros
    ));
    out.push_str(&format!(
        "latency_p99_micros:  {}
",
        l.p99_micros
    ));
    out.push_str(&format!(
        "latency_max_micros:  {}
",
        l.max_micros
    ));
    out
}

/// 有界解码一致性的确定性 ASCII 报告。
fn render_bounded_text(report: &BoundedDecodeReport) -> String {
    let m = &report.metrics;
    let mut out = String::new();
    out.push_str(&format!("schema:              {}\n", report.schema));
    out.push_str(&format!("scorer:              {}\n", report.scorer));
    out.push_str(&format!("beam_width:          {}\n", report.beam_width));
    out.push_str(&format!("top_k:               {}\n", report.top_k));
    out.push_str(&format!(
        "min_confidence_gap:  {}\n",
        report.min_confidence_gap
    ));
    out.push_str(&format!("compared:            {}\n", m.compared));
    out.push_str(&format!("top1_agreement:      {}\n", m.top1_agreement));
    out.push_str(&format!("truncated:           {}\n", m.truncated));
    out.push_str(&format!("low_confidence:      {}\n", m.low_confidence));
    out.push_str(&format!("empty:               {}\n", m.empty));
    out.push_str(&format!(
        "top1_agreement_rate: {:.4}\n",
        m.top1_agreement_rate()
    ));
    out.push_str(&format!("truncated_rate:      {:.4}\n", m.truncated_rate()));
    out
}

/// 与 committed 基线精确比较非计时指标。
fn check_baseline(
    report: &ContextReplayReport,
    text: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let expected: serde_json::Value = serde_json::from_str(text)?;
    let schema = expected
        .get("schema")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if schema != xhup_analyzer::context_replay::CONTEXT_REPLAY_BASELINE_SCHEMA {
        return Err(format!(
            "baseline schema 应为 {:?},实际为 {schema:?}",
            xhup_analyzer::context_replay::CONTEXT_REPLAY_BASELINE_SCHEMA
        )
        .into());
    }
    let m = &report.metrics;
    let actual = [
        ("sentences", m.sentences),
        ("tokens", m.tokens),
        ("ambiguous", m.ambiguous),
        ("baselineRank1", m.baseline_rank1),
        ("contextualRank1", m.contextual_rank1),
        ("contextGain", m.context_gain),
        ("harmfulReorder", m.harmful_reorder),
        (
            "baselineSelectionCostQ10",
            m.baseline_selection_cost_q10 as usize,
        ),
        (
            "contextualSelectionCostQ10",
            m.contextual_selection_cost_q10 as usize,
        ),
    ];
    let metrics = expected
        .get("metrics")
        .ok_or("baseline 缺少 metrics 字段")?;
    let mut failures = Vec::new();
    for (name, value) in actual {
        let want = metrics.get(name).and_then(serde_json::Value::as_u64);
        match want {
            Some(want) if want as usize == value => {}
            Some(want) => failures.push(format!("{name}: expected {want}, actual {value}")),
            None => failures.push(format!("{name}: baseline 缺少该指标")),
        }
    }
    Ok(failures)
}

#[cfg(test)]
mod tests {
    use super::*;
    use xhup_analyzer::context_replay::{
        ContextReplayMetrics, ContextReplayReport, replay_sentences_kdconv,
    };

    const SENTENCES: &str = include_str!("../../../../data/corpus/replay_fixture.txt");
    const BIGRAM: &str = include_str!("../../../../data/corpus/kdconv_bigram.tsv");

    fn report() -> ContextReplayReport {
        replay_sentences_kdconv(
            SENTENCES,
            xhup_decoder::BigramModel::from_tsv(BIGRAM).expect("bigram TSV 可解析"),
        )
    }

    /// 门禁必须能**通过**:committed 基线应与当前读数一致。
    #[test]
    fn committed_baseline_passes() {
        let baseline = include_str!("../../../../data/benchmarks/context-replay-baseline.json");
        let failures = check_baseline(&report(), baseline).expect("基线 JSON 合法");
        assert!(
            failures.is_empty(),
            "committed 基线应与当前读数一致,实际差异: {failures:?}"
        );
    }

    /// 门禁必须能**失败**:任一指标漂移都要被检出(否则门禁形同虚设)。
    #[test]
    fn metric_drift_is_detected() {
        let baseline = include_str!("../../../../data/benchmarks/context-replay-baseline.json");
        let good = report();
        // 构造漂移:把报告的每个受门禁指标分别改一位,逐一断言被检出。
        let mut cases: Vec<(String, ContextReplayMetrics)> = Vec::new();
        let mut bumped = good.metrics;
        bumped.harmful_reorder += 1;
        cases.push(("harmfulReorder".into(), bumped));
        let mut bumped = good.metrics;
        bumped.contextual_rank1 += 1;
        cases.push(("contextualRank1".into(), bumped));
        let mut bumped = good.metrics;
        bumped.context_gain += 1;
        cases.push(("contextGain".into(), bumped));
        let mut bumped = good.metrics;
        bumped.tokens += 1;
        cases.push(("tokens".into(), bumped));

        for (name, metrics) in cases {
            let drifted = ContextReplayReport { metrics, ..good };
            let failures = check_baseline(&drifted, baseline).expect("基线 JSON 合法");
            assert!(
                failures.iter().any(|f| f.contains(&name)),
                "指标 {name} 漂移必须被门禁检出,实际差异: {failures:?}"
            );
        }
    }

    /// 基线 schema 不对时必须报错,而不是静默通过。
    #[test]
    fn wrong_schema_is_rejected() {
        let bad = r#"{"schema":"some-other/v1","metrics":{}}"#;
        assert!(check_baseline(&report(), bad).is_err());
    }

    /// 缺少 metrics 字段时必须报错。
    #[test]
    fn missing_metrics_is_rejected() {
        let bad = r#"{"schema":"xhup-context-replay-baseline/v1"}"#;
        assert!(check_baseline(&report(), bad).is_err());
    }

    /// 基线缺少某个受门禁指标时必须报错(不能当作「无要求」)。
    #[test]
    fn missing_metric_is_reported() {
        let baseline = include_str!("../../../../data/benchmarks/context-replay-baseline.json");
        let mut value: serde_json::Value = serde_json::from_str(baseline).expect("合法 JSON");
        value["metrics"]
            .as_object_mut()
            .expect("metrics 为对象")
            .remove("harmfulReorder");
        let text = serde_json::to_string(&value).expect("可序列化");
        let failures = check_baseline(&report(), &text).expect("仍应可解析");
        assert!(
            failures.iter().any(|f| f.contains("harmfulReorder")),
            "基线缺指标必须显式报出: {failures:?}"
        );
    }
}
