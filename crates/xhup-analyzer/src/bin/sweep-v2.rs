//! `sweep-v2`:optimizer v2 参数扫描命令行(纯分析工具,不写仓库)。
//!
//! 用法:`sweep-v2 [--reference <参考映射TSV>] [--input <语料目录或 .txt>]
//! [--output <TSV 路径>] [--limit <N>]`
//!
//! 对 docs/optimizer-v2.md §5 的编译期网格逐运行点产出 v2 映射与五类
//! 指标(回放成本 / XHUP 兼容率 / top-N rank 分布 / fanout / 稳定性),
//! 机器可读 TSV 写 --output(缺省 stdout),人类可读摘要写 stderr。
//!
//! - 候选语法:`MONOTONE_V2_THEORETICAL`(研究语法,见 candidates.rs);
//! - 回放语料:未给 --input 时用 KdConv 聚合统计近似回放(原始句子不
//!   入库,见 data/corpus/README.md;指标为近似语义);
//! - 参考映射 TSV 本地可选(许可红线:绝不入库);缺省时兼容率列显式
//!   `NA`,XHUP 先验全部中立。

use std::path::PathBuf;
use std::process::ExitCode;

use xhup_analyzer::compat::parse_reference_tsv;
use xhup_analyzer::corpus::CorpusStats;
use xhup_analyzer::sweep_v2::{
    self, ReplaySource, SweepV2Input, render_summary, render_tsv, run_sweep_v2,
};
use xhup_analyzer::{BaselineMassView, CandidateEnumerationSpec, LexicalEvidenceSet};

/// 与 evidence.rs 同源的 KdConv 会话域聚合统计(默认回放语料)。
const CONVERSATION_TSV: &str = include_str!("../../../../data/corpus/conversation_kdconv.tsv");

fn usage() -> ! {
    eprintln!(
        "用法: sweep-v2 [--reference <参考映射TSV>] [--input <语料目录或 .txt>]\n\
         \x20             [--output <TSV 路径>] [--limit <N>]\n\
         \n\
         --reference  本地参考映射 TSV(文本<TAB>码<TAB>排名);缺省时兼容率为 NA。\n\
         --input      句子语料(每行一句;目录递归 .txt);缺省时用 KdConv 聚合\n\
         \x20            统计近似回放。\n\
         --output     机器可读 TSV 输出路径;缺省写 stdout。\n\
         --limit      只跑网格前 N 个运行点(冒烟用)。"
    );
    std::process::exit(2);
}

/// 与 replay-bench 相同的语料文件收集(单文件或目录递归 .txt,路径序)。
fn collect_txt_files(input: &std::path::Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    if input.is_file() {
        files.push(input.to_path_buf());
    } else if input.is_dir() {
        let mut stack = vec![input.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let mut entries: Vec<_> = std::fs::read_dir(&dir)
                .map_err(|e| format!("目录不可读 {}: {e}", dir.display()))?
                .map(|e| e.expect("目录项可读").path())
                .collect();
            entries.sort();
            for entry in entries {
                if entry.is_dir() {
                    stack.push(entry);
                } else if entry.extension().is_some_and(|ext| ext == "txt") {
                    files.push(entry);
                }
            }
        }
        files.sort();
    } else {
        return Err(format!("输入不存在: {}", input.display()));
    }
    if files.is_empty() {
        return Err(format!("输入中没有 .txt 语料文件: {}", input.display()));
    }
    Ok(files)
}

fn main() -> ExitCode {
    let mut reference_path: Option<PathBuf> = None;
    let mut input_path: Option<PathBuf> = None;
    let mut output_path: Option<PathBuf> = None;
    let mut limit: Option<usize> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--reference" => {
                reference_path = Some(PathBuf::from(args.next().unwrap_or_else(|| usage())))
            }
            "--input" => input_path = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            "--output" => output_path = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            "--limit" => {
                let value = args.next().unwrap_or_else(|| usage());
                limit = Some(value.parse().unwrap_or_else(|_| usage()));
            }
            _ => usage(),
        }
    }

    // 参考映射(本地可选;解析失败即失败,不静默降级)。
    let reference = match &reference_path {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("参考映射不可读 {}: {e}", path.display()));
            Some(parse_reference_tsv(&text).unwrap_or_else(|e| panic!("参考映射解析失败: {e}")))
        }
        None => None,
    };

    // 回放语料:--input 句子语料优先,缺省 KdConv 聚合统计近似。
    let sentences: Option<Vec<String>> = input_path.map(|path| {
        let files = collect_txt_files(&path).unwrap_or_else(|e| panic!("{e}"));
        let mut lines = Vec::new();
        for file in &files {
            let text = std::fs::read_to_string(file).expect("语料文件可读");
            lines.extend(text.lines().map(str::to_string));
        }
        lines
    });
    let aggregate = CorpusStats::from_tsv(CONVERSATION_TSV).expect("嵌入的语料统计必须可解析");

    // 分析输入(构建一次,全部运行点复用)。
    let data =
        xhup_analyzer::build_analysis_with_spec(CandidateEnumerationSpec::MONOTONE_V2_THEORETICAL);
    let evidence_set = LexicalEvidenceSet::build(&data.words, &data.frequency);
    let evidence = sweep_v2::evidence_by_word(&evidence_set);
    let baseline = BaselineMassView::build(&data.occupancy);

    let replay = match &sentences {
        Some(lines) => ReplaySource::Sentences(lines),
        None => ReplaySource::AggregateStats(&aggregate),
    };
    if sentences.is_none() {
        eprintln!("[sweep-v2] 未提供 --input,回放使用 KdConv 聚合统计近似(无句子上下文)");
    }
    if reference.is_none() {
        eprintln!("[sweep-v2] 未提供 --reference,兼容率指标为 NA,XHUP 先验中立");
    }

    let input = SweepV2Input {
        targets: &data.targets,
        evidence: &evidence,
        baseline: &baseline,
        replay,
        reference: reference.as_deref(),
    };
    let points = sweep_v2::grid();
    let points = match limit {
        Some(n) => &points[..n.min(points.len())],
        None => &points[..],
    };
    eprintln!(
        "[sweep-v2] 运行点: {} / {}",
        points.len(),
        sweep_v2::grid().len()
    );
    let rows = run_sweep_v2(&input, points);

    let tsv = render_tsv(&rows);
    match &output_path {
        Some(path) => std::fs::write(path, &tsv).expect("TSV 输出可写"),
        None => print!("{tsv}"),
    }
    eprint!("{}", render_summary(&rows));
    ExitCode::SUCCESS
}
