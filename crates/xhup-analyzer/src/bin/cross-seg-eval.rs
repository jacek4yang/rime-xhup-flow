//! `cross-seg-eval`:跨切分(码级)评测(Issue #83 §20,§25 第 6 步收益验收)。
//!
//! 对真实语料逐句构造生产 lattice 并做有界解码,度量 top1/top-k **分段**
//! 正确率。排除分类显式输出(不可达 ≠ 排序错误)。

use std::path::PathBuf;
use std::process::ExitCode;

use xhup_analyzer::cross_segmentation::{CrossSegmentationReport, evaluate_cross_segmentation};
use xhup_decoder::{BigramModel, KdconvBigramScorer};

fn usage() -> ! {
    eprintln!(
        "用法: cross-seg-eval --sentences <replay_fixture.txt> [--bigram <tsv>] [--top-k N] [--json]\n\
         在真实语料上度量跨切分(码级)top1/top-k 分段正确率。\n\
         --bigram 提供时用 KdconvBigramScorer,否则用 BaselineScorer(对照)。"
    );
    std::process::exit(2);
}

fn render(report: &CrossSegmentationReport) -> String {
    let m = &report.metrics;
    let mut out = String::new();
    out.push_str(&format!(
        "schema:                        {}\n",
        report.schema
    ));
    out.push_str(&format!(
        "scorer:                        {}\n",
        report.scorer
    ));
    out.push_str(&format!(
        "top_k:                         {}\n",
        report.top_k
    ));
    out.push_str(&format!(
        "beam:                          {} -> {}\n",
        report.beam_floor, report.beam_ceiling
    ));
    out.push_str(&format!("sentences:                     {}\n", m.sentences));
    out.push_str(&format!(
        "skipped_no_word_code:          {}\n",
        m.skipped_no_word_code
    ));
    out.push_str(&format!(
        "skipped_input_bounds:          {}\n",
        m.skipped_input_bounds
    ));
    out.push_str(&format!(
        "skipped_expected_path_absent:  {}\n",
        m.skipped_expected_path_absent
    ));
    out.push_str(&format!("evaluated:                     {}\n", m.evaluated));
    out.push_str(&format!(
        "top1_text_correct:             {}\n",
        m.top1_text_correct
    ));
    out.push_str(&format!(
        "top1_span_exact:               {}\n",
        m.top1_span_exact
    ));
    out.push_str(&format!("in_top_k:                      {}\n", m.in_top_k));
    out.push_str(&format!(
        "truncated_at_max:              {}\n",
        m.truncated_at_max
    ));
    out.push_str(&format!(
        "top1_text_rate:                {:.4}\n",
        m.top1_text_rate()
    ));
    out.push_str(&format!(
        "top1_span_exact_rate:          {:.4}\n",
        m.top1_span_exact_rate()
    ));
    out.push_str(&format!(
        "top_k_rate:                    {:.4}\n",
        m.top_k_rate()
    ));
    out.push_str(&format!(
        "exclusion_rate:                {:.4}\n",
        m.exclusion_rate()
    ));
    out
}

fn main() -> ExitCode {
    let mut sentences: Option<PathBuf> = None;
    let mut bigram: Option<PathBuf> = None;
    let mut top_k = 5usize;
    let mut json = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--sentences" => {
                sentences = Some(PathBuf::from(args.next().unwrap_or_else(|| usage())))
            }
            "--bigram" => bigram = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            "--json" => json = true,
            "--top-k" => {
                let value = args.next().unwrap_or_else(|| usage());
                top_k = match value.parse() {
                    Ok(parsed) if parsed > 0 => parsed,
                    _ => {
                        eprintln!("--top-k 必须 >= 1,实际 {value:?}");
                        return ExitCode::FAILURE;
                    }
                };
            }
            _ => usage(),
        }
    }
    let Some(sentences) = sentences else { usage() };

    let text = match std::fs::read_to_string(&sentences) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("无法读取语料 {}: {error}", sentences.display());
            return ExitCode::FAILURE;
        }
    };

    let report = match bigram {
        None => {
            xhup_analyzer::cross_segmentation::evaluate_cross_segmentation_baseline(&text, top_k)
        }
        Some(path) => {
            let bigram_text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(error) => {
                    eprintln!("无法读取 bigram {}: {error}", path.display());
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
            let scorer = KdconvBigramScorer::new(model);
            evaluate_cross_segmentation(&text, &scorer, KdconvBigramScorer::SCORER_ID, top_k)
        }
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("CrossSegmentationReport 可序列化")
        );
    } else {
        print!("{}", render(&report));
    }
    ExitCode::SUCCESS
}
