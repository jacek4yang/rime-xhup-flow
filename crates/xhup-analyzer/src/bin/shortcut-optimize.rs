//! `shortcut-optimize`:XHUP Flow 词语简码分析/优化模拟命令行工具。
//!
//! 纯分析工具:不修改任何 production 产物;转储文件由调用方指定路径,
//! 不写入仓库。

use std::process::ExitCode;
use std::time::Instant;

use xhup_analyzer::candidates::enumerate_targets;
use xhup_analyzer::frequency::FrequencyModel;
use xhup_analyzer::occupancy::CodeOccupancy;
use xhup_analyzer::optimize::{OptimizationProfile, optimize};
use xhup_analyzer::report::{
    balanced_cost, balanced_scale, dump_candidates_tsv, dump_recommendations_tsv,
};
use xhup_analyzer::sweep::run_sweep;
use xhup_analyzer::{AnalysisData, Timings, render_report};

fn usage() -> ! {
    eprintln!(
        "用法: shortcut-optimize [选项]\n\
         \n\
         选项:\n\
         \x20 --profile <zero-regression|fixed-first|optimized|empty-length-only>\n\
         \x20                     只运行指定 profile(默认:全部)\n\
         \x20 --format <text|tsv>   stdout 输出格式(默认 text 报告;tsv = 推荐表)\n\
         \x20 --dump-candidates <path>     转储全部候选 TSV(balanced 模型)\n\
         \x20 --dump-recommendations <path> 转储推荐结果 TSV(含稳健性)\n\
         \n\
         分析产物不进入码表;请输出到临时路径,不要 commit。"
    );
    std::process::exit(2);
}

fn parse_profile(name: &str) -> OptimizationProfile {
    match name {
        "zero-regression" => OptimizationProfile::ZeroRegression,
        "fixed-first" => OptimizationProfile::FixedFirst,
        "optimized" => OptimizationProfile::Optimized,
        "empty-length-only" => OptimizationProfile::EmptyLengthOnly,
        _ => usage(),
    }
}

fn main() -> ExitCode {
    let mut profile_filter: Option<OptimizationProfile> = None;
    let mut format_tsv = false;
    let mut dump_candidates_path: Option<String> = None;
    let mut dump_recommendations_path: Option<String> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--profile" => {
                let value = args.next().unwrap_or_else(|| usage());
                profile_filter = Some(parse_profile(&value));
            }
            "--format" => match args.next().unwrap_or_else(|| usage()).as_str() {
                "text" => format_tsv = false,
                "tsv" => format_tsv = true,
                _ => usage(),
            },
            "--dump-candidates" => {
                dump_candidates_path = Some(args.next().unwrap_or_else(|| usage()));
            }
            "--dump-recommendations" => {
                dump_recommendations_path = Some(args.next().unwrap_or_else(|| usage()));
            }
            "--help" | "-h" => usage(),
            _ => usage(),
        }
    }

    // 分阶段计时构建不可变分析输入(全部 sweep 运行复用)。
    let start = Instant::now();
    let chars = xhup_generator::char_code_analysis_entries();
    let words = xhup_generator::word_code_analysis_entries();
    let load_evidence = start.elapsed();

    let start = Instant::now();
    let occupancy = CodeOccupancy::build();
    let build_occupancy = start.elapsed();

    let start = Instant::now();
    let (targets, enumeration) = enumerate_targets(&words);
    let enumeration_elapsed = start.elapsed();

    let frequency = FrequencyModel::build(&chars, &words);
    let data = AnalysisData {
        chars,
        words,
        occupancy,
        targets,
        enumeration,
        frequency,
    };
    eprintln!(
        "分析输入:{} targets / {} candidates",
        data.targets.len(),
        data.enumeration.actual
    );

    // 单次优化运行计时(balanced / ZERO_REGRESSION)。
    let start = Instant::now();
    let scale = balanced_scale();
    let cost = balanced_cost();
    let single = optimize(
        &data.targets,
        &data.occupancy,
        &data.frequency,
        &scale,
        &cost,
        OptimizationProfile::ZeroRegression,
    );
    let single_run = start.elapsed();
    eprintln!(
        "单次运行(ZERO_REGRESSION balanced):{} 条推荐",
        single.assignments.len()
    );

    // sweep。
    let profiles: Vec<OptimizationProfile> = match profile_filter {
        Some(profile) => vec![profile],
        None => OptimizationProfile::all().to_vec(),
    };
    let start = Instant::now();
    let runs = run_sweep(&data.targets, &data.occupancy, &data.frequency, &profiles);
    let sweep = start.elapsed();
    eprintln!("sweep 完成:{} 次运行", runs.len());

    // 转储。
    if let Some(path) = dump_candidates_path {
        let start = Instant::now();
        let tsv = dump_candidates_tsv(&data);
        if let Err(error) = std::fs::write(&path, tsv) {
            eprintln!("写入 {path} 失败:{error}");
            return ExitCode::FAILURE;
        }
        eprintln!("候选转储 → {path}({})", fmt_note(start.elapsed()));
    }
    if let Some(path) = dump_recommendations_path {
        let tsv = dump_recommendations_tsv(&runs);
        if let Err(error) = std::fs::write(&path, tsv) {
            eprintln!("写入 {path} 失败:{error}");
            return ExitCode::FAILURE;
        }
        eprintln!("推荐转储 → {path}");
    }

    let timings = Timings {
        load_evidence,
        build_occupancy,
        enumeration: enumeration_elapsed,
        single_run,
        sweep,
    };
    if format_tsv {
        print!("{}", dump_recommendations_tsv(&runs));
    } else {
        print!("{}", render_report(&data, &runs, &timings));
    }
    ExitCode::SUCCESS
}

fn fmt_note(d: std::time::Duration) -> String {
    format!("{:.3}s", d.as_secs_f64())
}
