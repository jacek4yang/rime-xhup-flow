//! `replay-bench`:语料回放基准命令行(纯分析工具,不写仓库)。
//!
//! 用法:`replay-bench --input <语料目录或 .txt 文件(每行一句)>`
//!
//! 用当前 canonical 映射回放语料,输出确定性报告(KSPC / rank1 /
//! rank≤3 / 期望成本 / 兜底率)。用于 optimizer v2 工作点比较与
//! 映射变更门禁(docs/optimizer-v2.md §5)。

use std::path::PathBuf;
use std::process::ExitCode;

use xhup_analyzer::replay::{ReplayCostModel, Replayer};

fn usage() -> ! {
    eprintln!(
        "用法: replay-bench --input <语料目录或 .txt 文件>\n\
         \n\
         输入为 UTF-8 纯文本,每行一句(目录则递归读取全部 .txt,路径序)。\n\
         输出当前 canonical 映射的静态层回放指标(KSPC/rank1/top3/成本)。"
    );
    std::process::exit(2);
}

fn main() -> ExitCode {
    let mut input: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--input" => input = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            _ => usage(),
        }
    }
    let Some(input) = input else {
        usage();
    };

    // 与 corpus-stats 相同的输入收集(单文件或目录递归 .txt)。
    let mut files = Vec::new();
    if input.is_file() {
        files.push(input.clone());
    } else if input.is_dir() {
        let mut stack = vec![input.clone()];
        while let Some(dir) = stack.pop() {
            let mut entries: Vec<_> = std::fs::read_dir(&dir)
                .expect("目录可读")
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
        eprintln!("输入不存在: {}", input.display());
        return ExitCode::FAILURE;
    }
    if files.is_empty() {
        eprintln!("输入中没有 .txt 语料文件");
        return ExitCode::FAILURE;
    }

    let replayer = Replayer::new(&ReplayCostModel::default());
    let mut lines: Vec<String> = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("语料文件可读");
        lines.extend(text.lines().map(str::to_string));
    }
    let report = replayer.replay_corpus(lines.iter().map(String::as_str));

    println!("语料回放报告(当前 canonical 映射,静态层)");
    println!("文件: {}  句子: {}", files.len(), report.sentences);
    println!(
        "汉字: {}  token: {}",
        report.totals.chars, report.totals.tokens
    );
    println!("键数: {}", report.totals.keys);
    println!("KSPC(键/字): {:.3}", report.kspc());
    println!("rank1 命中率: {:.1}%", report.rank1_rate() * 100.0);
    println!("rank≤3 命中率: {:.1}%", report.top3_rate() * 100.0);
    println!("期望成本合计: {:.1}", report.totals.expected_cost);
    println!(
        "兜底 token: {} ({:.2}%)",
        report.totals.fallback_tokens,
        report.totals.fallback_tokens as f64 / report.totals.tokens.max(1) as f64 * 100.0
    );
    ExitCode::SUCCESS
}
