//! `context-replay-bench`:真实语料 committed-context 回放基准(Issue #83 §20/§24)。
//!
//! 对真实句子语料逐 token 回放,比较 baseline(忽略上下文)与 KDConv bigram
//! scorer 的 rank1 命中与有害重排率。可选 `--baseline` 做回归门禁断言。
//!
//! 全离线:只读显式传入的文件,不联网、不写用户数据。

use std::path::PathBuf;
use std::process::ExitCode;

use xhup_analyzer::context_replay::{ContextReplayReport, replay_sentences_kdconv};
use xhup_decoder::BigramModel;

fn usage() -> ! {
    eprintln!(
        "用法: context-replay-bench --sentences <replay_fixture.txt> --bigram <kdconv_bigram.tsv>\n\
         \x20     [--json] [--baseline <context-replay-baseline.json>]\n\
         对真实语料逐 token 回放,输出 committed-context 的 rank1 命中与\n\
         harmful reorder rate(§20)。baseline 只断言非计时指标。"
    );
    std::process::exit(2);
}

fn main() -> ExitCode {
    let mut sentences: Option<PathBuf> = None;
    let mut bigram: Option<PathBuf> = None;
    let mut baseline: Option<PathBuf> = None;
    let mut json = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--sentences" => {
                sentences = Some(PathBuf::from(args.next().unwrap_or_else(|| usage())))
            }
            "--bigram" => bigram = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            "--baseline" => baseline = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            "--json" => json = true,
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

    let report = replay_sentences_kdconv(&sentences_text, model);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("ContextReplayReport 必然可序列化")
        );
    } else {
        print!("{}", render_text(&report));
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
