//! `corpus-stats`:语料统计管线命令行(纯分析工具,不写仓库)。
//!
//! 用法:`corpus-stats --input <语料目录或文件> --output <统计 TSV 路径>`
//!
//! 输入:UTF-8 纯文本,每行一句(目录则递归读取全部 `.txt` 文件,
//! 按路径字典序处理,确定性)。输出:派生统计 TSV(见 corpus 模块;
//! 聚合计数可再分发,原句绝不外带)。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use xhup_analyzer::corpus::CorpusStatsBuilder;

fn usage() -> ! {
    eprintln!(
        "用法: corpus-stats --input <语料目录或 .txt 文件> --output <统计 TSV 路径>\n\
         \n\
         输入为 UTF-8 纯文本,每行一句;目录输入递归读取全部 .txt(路径序)。\n\
         输出为确定性派生统计 TSV(word/count/sentences/left/right contexts)。"
    );
    std::process::exit(2);
}

/// 收集输入文件列表(单文件或目录递归 .txt,确定性字典序)。
fn collect_inputs(path: &Path) -> Result<Vec<PathBuf>, String> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    if !path.is_dir() {
        return Err(format!("输入不存在: {}", path.display()));
    }
    let mut files = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map_err(|e| format!("无法读取目录 {}: {e}", dir.display()))?
            .map(|entry| entry.expect("目录项可读").path())
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
    Ok(files)
}

fn main() -> ExitCode {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--input" => input = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            "--output" => output = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            _ => usage(),
        }
    }
    let (Some(input), Some(output)) = (input, output) else {
        usage();
    };

    let files = match collect_inputs(&input) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    if files.is_empty() {
        eprintln!("输入中没有 .txt 语料文件");
        return ExitCode::FAILURE;
    }

    let mut builder = CorpusStatsBuilder::new();
    for file in &files {
        let text = match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(e) => {
                eprintln!("无法读取 {}: {e}", file.display());
                return ExitCode::FAILURE;
            }
        };
        for line in text.lines() {
            builder.feed(line);
        }
    }
    let stats = builder.finish();
    if let Err(e) = std::fs::write(&output, stats.to_tsv()) {
        eprintln!("无法写出 {}: {e}", output.display());
        return ExitCode::FAILURE;
    }
    println!(
        "语料统计完成:{} 个文件,{} 句,{} token,{} 词 → {}",
        files.len(),
        stats.sentences,
        stats.tokens,
        stats.words.len(),
        output.display()
    );
    ExitCode::SUCCESS
}
