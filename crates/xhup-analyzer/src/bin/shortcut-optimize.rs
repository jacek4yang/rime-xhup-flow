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
         \x20 --dump-production-zero-regression <path>\n\
         \x20                     导出 production ZERO_REGRESSION 简码 canonical TSV\n\
         \x20                     (policy zero-regression-high-v1;只跑 ZR 主网格,\n\
         \x20                     导出后退出,不输出完整报告)\n\
         \x20 --audit-prefix        词语简码层 prefix 拓扑全量静态审计\n\
         \x20                     (含各码长 runtime 哨兵;打印后退出)\n\
         \n\
         production 导出是 canonical 生产数据:入库需 diff review 与 policy review。\n\
         其余分析产物不进入码表;请输出到临时路径,不要 commit。"
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
    let mut dump_production_path: Option<String> = None;
    let mut audit_prefix = false;

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
            "--dump-production-zero-regression" => {
                dump_production_path = Some(args.next().unwrap_or_else(|| usage()));
            }
            "--audit-prefix" => audit_prefix = true,
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
    let occupancy = CodeOccupancy::build_baseline_fixed();
    let build_occupancy = start.elapsed();

    // prefix 拓扑审计快速路径:只需 baseline occupancy,不跑候选枚举与优化。
    if audit_prefix {
        let audit = xhup_analyzer::audit_prefix_topology(&occupancy);
        eprint!("{}", render_prefix_audit(&audit));
        return ExitCode::SUCCESS;
    }

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

    // production 导出快速路径:只跑 ZERO_REGRESSION 的 30 次 normalized
    // 主网格运行(reference 与 robustness 同源),不跑完整 sweep。
    if let Some(path) = dump_production_path {
        let start = Instant::now();
        let evidence = xhup_analyzer::collect_evidence(&data);
        let selection = xhup_analyzer::select_production_shortcuts(&evidence, &data.occupancy);
        let benefit = xhup_analyzer::benefit_audit(&data, &evidence, &selection);
        let tsv = xhup_analyzer::serialize_canonical_tsv(&selection.selected);
        if let Err(error) = std::fs::write(&path, tsv) {
            eprintln!("写入 {path} 失败:{error}");
            return ExitCode::FAILURE;
        }
        eprint!(
            "{}",
            render_production_audit(&path, &selection, &benefit, start.elapsed())
        );
        return ExitCode::SUCCESS;
    }

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

/// production 导出审计摘要(selection / 收益 / 长度 / 模式 / top 列表)。
fn render_production_audit(
    path: &str,
    selection: &xhup_analyzer::ProductionSelection,
    benefit: &xhup_analyzer::BenefitAudit,
    elapsed: std::time::Duration,
) -> String {
    use std::fmt::Write as _;
    use xhup_analyzer::ExclusionReason as Reason;

    let audit = &selection.audit;
    let mut out = String::new();
    writeln!(
        out,
        "production ZERO_REGRESSION 导出 → {path}(policy {};{})",
        xhup_analyzer::PRODUCTION_SHORTCUT_POLICY_VERSION,
        fmt_note(elapsed)
    )
    .unwrap();
    writeln!(out, "selection audit:").unwrap();
    writeln!(
        out,
        "  reference assignments: {}",
        audit.reference_assignments
    )
    .unwrap();
    writeln!(out, "  robustness records:    {}", audit.robustness_records).unwrap();
    writeln!(out, "  selected production:   {}", audit.selected).unwrap();
    writeln!(
        out,
        "  excluded: NO_ROBUSTNESS_EVIDENCE={} TOP_CODE_MISMATCH={} BELOW_THRESHOLD={} BASELINE_OCCUPIED={}",
        audit.excluded_by(Reason::NoRobustnessEvidence),
        audit.excluded_by(Reason::TopCodeMismatch),
        audit.excluded_by(Reason::BelowThreshold),
        audit.excluded_by(Reason::BaselineOccupied),
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "weighted benefit(reference 尺度,无量纲):").unwrap();
    writeln!(
        out,
        "  weighted keys before:     {:.6e}",
        benefit.weighted_keys_before
    )
    .unwrap();
    writeln!(
        out,
        "  full ZR keys saved:       {:.6e}",
        benefit.full_zr_keys_saved
    )
    .unwrap();
    writeln!(
        out,
        "  production keys saved:    {:.6e}",
        benefit.production_keys_saved
    )
    .unwrap();
    writeln!(
        out,
        "  full ZR saving:           {:.2}%",
        100.0 * benefit.full_zr_keys_saved / benefit.weighted_keys_before
    )
    .unwrap();
    writeln!(
        out,
        "  production saving:        {:.2}%",
        100.0 * benefit.production_keys_saved / benefit.weighted_keys_before
    )
    .unwrap();
    writeln!(
        out,
        "  retained benefit ratio:   {:.2}%",
        100.0 * benefit.retained_ratio()
    )
    .unwrap();
    writeln!(out).unwrap();

    // 长度审计。
    writeln!(out, "production shortcut lengths:").unwrap();
    for length in 3..=7 {
        let count = selection
            .selected
            .iter()
            .filter(|e| e.shortcut_code.len() == length)
            .count();
        let mass: u64 = selection
            .selected
            .iter()
            .filter(|e| e.shortcut_code.len() == length)
            .map(|e| e.frequency_score)
            .sum();
        writeln!(out, "  {length}-key: {count} 条(frequency mass {mass})").unwrap();
    }
    writeln!(out).unwrap();

    // 模式审计(按条数降序)。
    let mut patterns: std::collections::BTreeMap<&str, (usize, u64)> =
        std::collections::BTreeMap::new();
    for entry in &selection.selected {
        let slot = patterns.entry(entry.mode.as_str()).or_default();
        slot.0 += 1;
        slot.1 += entry.frequency_score;
    }
    let mut patterns: Vec<(&str, (usize, u64))> = patterns.into_iter().collect();
    patterns.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(b.0)));
    writeln!(out, "production shortcut patterns:").unwrap();
    for (pattern, (count, mass)) in patterns {
        writeln!(out, "  {pattern}: {count} 条(frequency mass {mass})").unwrap();
    }
    writeln!(out).unwrap();

    // top 50 production(词频降序,词升序兜底)。
    let mut by_frequency = selection.selected.iter().collect::<Vec<_>>();
    by_frequency.sort_by(|a, b| {
        b.frequency_score
            .cmp(&a.frequency_score)
            .then(a.word.cmp(&b.word))
    });
    writeln!(out, "top 50 production shortcuts(by frequency):").unwrap();
    for (index, entry) in by_frequency.iter().take(50).enumerate() {
        writeln!(
            out,
            "  {:2}. {} {} {} {} score={} stability={}/{} saved={}",
            index + 1,
            entry.word,
            entry.full_code,
            entry.shortcut_code,
            entry.mode,
            entry.frequency_score,
            entry.top_code_votes,
            entry.total_runs,
            entry.full_code.len() - entry.shortcut_code.len(),
        )
        .unwrap();
    }
    writeln!(out).unwrap();

    // top 30 被排除高频 reference assignments。
    let mut excluded = selection.exclusions.iter().collect::<Vec<_>>();
    excluded.sort_by(|a, b| {
        b.frequency_score
            .cmp(&a.frequency_score)
            .then(a.word.cmp(&b.word))
    });
    writeln!(out, "top 30 excluded high-frequency assignments:").unwrap();
    for (index, exclusion) in excluded.iter().take(30).enumerate() {
        writeln!(
            out,
            "  {:2}. {} ref={} score={} stability={}/{} top_code={} reason={:?}",
            index + 1,
            exclusion.word,
            exclusion.reference_code,
            exclusion.frequency_score,
            exclusion.votes.0,
            exclusion.votes.1,
            exclusion.top_code.as_deref().unwrap_or("-"),
            exclusion.reason,
        )
        .unwrap();
    }
    out
}

/// prefix 拓扑审计渲染(全量统计 + 各码长 deterministic runtime 哨兵)。
fn render_prefix_audit(audit: &xhup_analyzer::PrefixAudit) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    writeln!(out, "词语简码层 prefix 拓扑审计(全量静态):").unwrap();
    writeln!(out, "  production shortcuts: {}", audit.shortcut_count).unwrap();
    writeln!(
        out,
        "  shortcut 是 baseline 码 strict prefix 的对数: {}",
        audit.shortcut_prefix_of_baseline_pairs
    )
    .unwrap();
    writeln!(
        out,
        "  其中 distinct shortcut 数:                  {}",
        audit.shortcuts_prefixing_baseline
    )
    .unwrap();
    writeln!(
        out,
        "  baseline 码是 shortcut strict prefix 的对数: {}",
        audit.baseline_prefix_of_shortcut_pairs
    )
    .unwrap();
    writeln!(
        out,
        "  shortcut 互为 strict prefix 的对数:          {}",
        audit.shortcut_to_shortcut_pairs
    )
    .unwrap();
    writeln!(out).unwrap();

    writeln!(out, "各码长 runtime 哨兵(deterministic):").unwrap();
    for length in &audit.lengths {
        if length.rows == 0 {
            writeln!(out, "  {}-key: 0 条(N/A)", length.length).unwrap();
            continue;
        }
        writeln!(out, "  {}-key: {} 条", length.length, length.rows).unwrap();
        let render = |label: &str, sentinel: Option<&xhup_analyzer::PrefixSentinel>| match sentinel
        {
            Some(s) => format!(
                "    {label}: {} {} {} {} score={}",
                s.word, s.full_code, s.shortcut_code, s.mode, s.frequency_score
            ),
            None => format!("    {label}: N/A"),
        };
        writeln!(
            out,
            "{}",
            render("lex-first       ", length.lex_first.as_ref())
        )
        .unwrap();
        writeln!(
            out,
            "{}",
            render("top-frequency   ", length.top_frequency.as_ref())
        )
        .unwrap();
        writeln!(
            out,
            "{}",
            render("prefix-own-full ", length.prefix_lex_first.as_ref())
        )
        .unwrap();
        writeln!(
            out,
            "{}",
            render("non-prefix-first", length.non_prefix_lex_first.as_ref())
        )
        .unwrap();
    }
    out
}
