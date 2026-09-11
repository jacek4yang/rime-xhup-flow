//! `contextual-bench`:版本化联合分段 benchmark 的确定性 baseline runner。

use std::path::PathBuf;
use std::process::ExitCode;

use xhup_analyzer::contextual_benchmark::{BenchmarkRunner, BenchmarkSuite, check_baseline_json};

fn usage() -> ! {
    eprintln!(
        "用法: contextual-bench --input <benchmark.json> [--baseline <baseline.json>]\n\
         输出词频+分段 baseline 的 JSON 报告；baseline 只断言非计时指标。"
    );
    std::process::exit(2);
}

fn main() -> ExitCode {
    let mut input: Option<PathBuf> = None;
    let mut baseline: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--input" => input = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            "--baseline" => baseline = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            _ => usage(),
        }
    }
    let Some(input) = input else {
        usage();
    };

    let text = match std::fs::read_to_string(&input) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("无法读取 benchmark {}: {error}", input.display());
            return ExitCode::FAILURE;
        }
    };
    let suite = match BenchmarkSuite::from_json(&text) {
        Ok(suite) => suite,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let report = BenchmarkRunner::default().run(&suite);
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("BenchmarkReport 必然可序列化")
    );

    if let Some(path) = baseline {
        let baseline_text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("无法读取 baseline {}: {error}", path.display());
                return ExitCode::FAILURE;
            }
        };
        match check_baseline_json(&report, &baseline_text) {
            Ok(failures) if failures.is_empty() => {
                eprintln!("contextual baseline 断言通过");
            }
            Ok(failures) => {
                eprintln!("contextual baseline 断言失败:");
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
