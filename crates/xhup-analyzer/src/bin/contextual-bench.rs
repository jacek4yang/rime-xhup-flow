//! `contextual-bench`:版本化联合分段 benchmark 的确定性 runner。
//!
//! 支持在 baseline(忽略上下文)与 kdconv bigram(使用 committed context)
//! 两个 scorer 上跑同一 fixture,并可输出跨 scorer 差分(context gain /
//! harmful reorder rate)。

use std::path::PathBuf;
use std::process::ExitCode;

use xhup_analyzer::contextual_benchmark::{
    BenchmarkRunner, BenchmarkSuite, check_baseline_json, compare_runs,
};
use xhup_decoder::{BaselineScorer, BigramModel, KdconvBigramScorer};

/// 从显式路径加载 kdconv bigram 模型。路径缺失或文件非法时返回错误退出码。
///
/// 刻意要求显式路径:不隐式读取生产数据,也不做网络回退(离线红线)。
fn load_bigram(path: &Option<PathBuf>) -> Result<BigramModel, ExitCode> {
    let Some(path) = path else {
        eprintln!("需要 --bigram <kdconv_bigram.tsv>(显式路径,不隐式读生产数据)");
        return Err(ExitCode::FAILURE);
    };
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("无法读取 bigram {}: {error}", path.display());
            return Err(ExitCode::FAILURE);
        }
    };
    match BigramModel::from_tsv(&text) {
        Ok(model) => Ok(model),
        Err(error) => {
            eprintln!("bigram TSV 非法: {error}");
            Err(ExitCode::FAILURE)
        }
    }
}

fn usage() -> ! {
    eprintln!(
        "用法: contextual-bench --input <benchmark.json>\n\
         \x20     [--baseline <baseline.json>] [--scorer baseline|kdconv-bigram]\n\
         \x20     [--bigram <kdconv_bigram.tsv>] [--compare-context]\n\
         输出词频+分段 scorer 的 JSON 报告;baseline 只断言非计时指标。\n\
         --scorer            报告使用哪个 scorer(缺省 baseline)\n\
         --bigram            kdconv bigram TSV(显式路径,不隐式读生产数据)\n\
         --compare-context   额外输出 baseline vs bigram 的 context gain /\n\
                             harmful reorder rate 差分"
    );
    std::process::exit(2);
}

fn main() -> ExitCode {
    let mut input: Option<PathBuf> = None;
    let mut baseline: Option<PathBuf> = None;
    let mut bigram: Option<PathBuf> = None;
    let mut scorer = String::from("baseline");
    let mut compare_context = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--input" => input = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            "--baseline" => baseline = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            "--bigram" => bigram = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            "--scorer" => scorer = args.next().unwrap_or_else(|| usage()),
            "--compare-context" => compare_context = true,
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
    // scorer 选择:baseline(忽略上下文)或 kdconv bigram(使用 committed
    // context)。A/B 差分由 --compare-context 开启,使用显式传入的 bigram
    // TSV,避免隐式读取生产数据。
    let run = match scorer.as_str() {
        "baseline" => BenchmarkRunner::<BaselineScorer>::default().run(&suite),
        "kdconv-bigram" => {
            let model = match load_bigram(&bigram) {
                Ok(model) => model,
                Err(code) => return code,
            };
            BenchmarkRunner::new(KdconvBigramScorer::new(model)).run(&suite)
        }
        other => {
            eprintln!("未知 scorer {other:?}(支持 baseline | kdconv-bigram)");
            return ExitCode::FAILURE;
        }
    };
    let report = run.report();
    println!(
        "{}",
        serde_json::to_string_pretty(report).expect("BenchmarkReport 必然可序列化")
    );

    if compare_context {
        let model = match load_bigram(&bigram) {
            Ok(model) => model,
            Err(code) => return code,
        };
        let baseline_run = BenchmarkRunner::<BaselineScorer>::default().run(&suite);
        let contextual_run = BenchmarkRunner::new(KdconvBigramScorer::new(model)).run(&suite);
        let comparison = compare_runs(&baseline_run, &contextual_run);
        print!("{}", comparison.render_ascii());
    }

    if let Some(path) = baseline {
        let baseline_text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("无法读取 baseline {}: {error}", path.display());
                return ExitCode::FAILURE;
            }
        };
        match check_baseline_json(report, &baseline_text) {
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
