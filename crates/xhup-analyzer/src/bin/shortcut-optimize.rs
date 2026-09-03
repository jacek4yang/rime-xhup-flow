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
         \x20 --dump-production-fixed-first <path>\n\
         \x20                     导出 production FIXED_FIRST 简码 canonical TSV\n\
         \x20                     (policy fixed-first-high-v1;incremental universe\n\
         \x20                     上跑 30 次 normalized 主网格,导出后退出)\n\
         \x20 --dump-fixed-first-audit-manifest <path>\n\
         \x20                     导出 FIXED_FIRST runtime A/B 审计 manifest\n\
         \x20                     (与 --dump-production-fixed-first 共享同一次证据)\n\
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
    let mut dump_fixed_first_path: Option<String> = None;
    let mut dump_fixed_first_manifest_path: Option<String> = None;
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
            "--dump-production-fixed-first" => {
                dump_fixed_first_path = Some(args.next().unwrap_or_else(|| usage()));
            }
            "--dump-fixed-first-audit-manifest" => {
                dump_fixed_first_manifest_path = Some(args.next().unwrap_or_else(|| usage()));
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

    // production FIXED_FIRST 导出快速路径:incremental universe(排除 ZR 词、
    // 只留 baseline fanout > 0 重码候选)上的 30 次 normalized 主网格运行,
    // reference 与 robustness 同源;TSV 与 runtime 审计 manifest 共享一次证据。
    if dump_fixed_first_path.is_some() || dump_fixed_first_manifest_path.is_some() {
        let start = Instant::now();
        let evidence = xhup_analyzer::collect_fixed_first_evidence(&data);
        // 兼容性断言参考系:baseline + ZERO_REGRESSION 层(不含 FF 层自身,
        // 否则导出自引用)。选择本身只基于 baseline + frozen ZR 集合。
        let pre_fixed_first_current = CodeOccupancy::build_pre_fixed_first_production();
        let selection = xhup_analyzer::select_fixed_first_production(
            &evidence,
            &data.occupancy,
            &pre_fixed_first_current,
        );
        let benefit = xhup_analyzer::fixed_first_benefit_audit(&data, &selection);
        if let Some(path) = dump_fixed_first_path {
            let tsv = xhup_analyzer::serialize_fixed_first_tsv(&selection.selected);
            if let Err(error) = std::fs::write(&path, tsv) {
                eprintln!("写入 {path} 失败:{error}");
                return ExitCode::FAILURE;
            }
            eprint!(
                "{}",
                render_fixed_first_audit(
                    &path,
                    &data,
                    &evidence,
                    &selection,
                    &benefit,
                    start.elapsed(),
                )
            );
        }
        if let Some(path) = dump_fixed_first_manifest_path {
            let manifest =
                xhup_analyzer::fixed_first_audit_manifest(&selection.selected, &data.occupancy);
            if let Err(error) = std::fs::write(&path, &manifest) {
                eprintln!("写入 {path} 失败:{error}");
                return ExitCode::FAILURE;
            }
            eprintln!(
                "FIXED_FIRST runtime 审计 manifest → {path}({} 行)",
                selection.selected.len()
            );
        }
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

/// nearest-rank 分位数(空集返回 0)。
fn percentile_nearest_rank(sorted: &[usize], p: usize) -> usize {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (p * sorted.len()).div_ceil(100);
    sorted[rank.max(1) - 1]
}

/// PR #22 ZERO_REGRESSION production 层的 raw 加权按键节省(与 FIXED_FIRST
/// 共用同一 100k universe denominator;由 canonical ZR 条目现算,不重跑
/// ZR sweep)。
fn zr_production_keys_saved(data: &AnalysisData) -> f64 {
    let scale = xhup_analyzer::reference_scale();
    let scores: std::collections::BTreeMap<(&str, &xhup_core::KeySequence), u64> = data
        .words
        .iter()
        .map(|entry| ((entry.word(), entry.code()), entry.frequency_score()))
        .collect();
    xhup_generator::canonical_word_shortcut_entries()
        .iter()
        .map(|entry| {
            let score = scores
                .get(&(entry.word(), entry.full_code()))
                .copied()
                .expect("不变量:ZR canonical (词, 完整码) 必然存在于词层");
            data.frequency.target_weight(&scale, score)
                * (entry.full_code().len() - entry.shortcut_code().len()) as f64
        })
        .sum()
}

/// production FIXED_FIRST 导出审计摘要(universe / selection / 收益 / 分布 /
/// top 列表 / 「时间」哨兵 / 与 ZR 层合并口径)。
fn render_fixed_first_audit(
    path: &str,
    data: &AnalysisData,
    evidence: &xhup_analyzer::FixedFirstEvidence,
    selection: &xhup_analyzer::FixedFirstProductionSelection,
    benefit: &xhup_analyzer::FixedFirstBenefitAudit,
    elapsed: std::time::Duration,
) -> String {
    use std::fmt::Write as _;
    use xhup_analyzer::FixedFirstExclusionReason as Reason;
    use xhup_core::KeySequence;

    let universe = &evidence.universe;
    let audit = &selection.audit;
    let mut out = String::new();
    writeln!(
        out,
        "production FIXED_FIRST 导出 → {path}(policy {};{})",
        xhup_analyzer::FIXED_FIRST_PRODUCTION_POLICY_VERSION,
        fmt_note(elapsed)
    )
    .unwrap();
    writeln!(
        out,
        "reference: {:?} × normalized(50:50, Conservative)(typed 选取,30 次主网格之一)",
        evidence.reference_point
    )
    .unwrap();
    writeln!(out).unwrap();

    writeln!(out, "incremental universe audit:").unwrap();
    writeln!(
        out,
        "  original word targets:                  {}",
        universe.original_targets
    )
    .unwrap();
    writeln!(
        out,
        "  ZR production words excluded(优化前): {}",
        universe.zr_words_excluded
    )
    .unwrap();
    writeln!(
        out,
        "  remaining targets:                      {}",
        universe.remaining_targets
    )
    .unwrap();
    writeln!(
        out,
        "  colliding candidates(baseline fanout > 0,进入优化): {}",
        universe.colliding_candidates
    )
    .unwrap();
    writeln!(
        out,
        "  targets left with no candidate:           {}",
        universe.targets_without_candidates
    )
    .unwrap();
    write!(out, "  candidate lengths:").unwrap();
    for length in 3..=7 {
        write!(
            out,
            " {}-key={}",
            length, universe.candidate_lengths[length]
        )
        .unwrap();
    }
    writeln!(out).unwrap();
    writeln!(out).unwrap();

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
        "  excluded total: {} (NO_ROBUSTNESS_EVIDENCE={} TOP_CODE_MISMATCH={} BELOW_THRESHOLD={} NON_POSITIVE_UTILITY={})",
        audit.excluded_total(),
        audit.excluded_by(Reason::NoRobustnessEvidence),
        audit.excluded_by(Reason::TopCodeMismatch),
        audit.excluded_by(Reason::BelowThreshold),
        audit.excluded_by(Reason::NonPositiveReferenceUtility),
    )
    .unwrap();
    writeln!(
        out,
        "  structural(理论 0): ALREADY_ZR_WORD={} NO_COLLIDING={} OCCUPANCY_MISMATCH={} ZR_CODE_CONFLICT={}",
        audit.excluded_by(Reason::AlreadyZeroRegressionWord),
        audit.excluded_by(Reason::NoCollidingCandidate),
        audit.excluded_by(Reason::CurrentOccupancyMismatch),
        audit.excluded_by(Reason::ZeroRegressionCodeConflict),
    )
    .unwrap();
    writeln!(out).unwrap();

    // 收益(raw 键数差 / 有效模型收益;combined 与 ZR 层共用同一 100k
    // universe denominator,不做百分比相加)。
    let zr_saved = zr_production_keys_saved(data);
    let before = benefit.weighted_keys_before;
    let ff_raw = benefit.production_raw_keys_saved;
    writeln!(
        out,
        "weighted benefit(reference 尺度,无量纲;denominator = 全部 100k 词目标):"
    )
    .unwrap();
    writeln!(out, "  weighted keys before:        {before:.6e}").unwrap();
    writeln!(
        out,
        "  FIXED_FIRST raw saved:       {ff_raw:.6e} ({:.2}%)",
        100.0 * ff_raw / before
    )
    .unwrap();
    writeln!(
        out,
        "  FIXED_FIRST effective saved: {:.6e} ({:.2}%)",
        benefit.production_effective_benefit,
        100.0 * benefit.production_effective_benefit / before
    )
    .unwrap();
    writeln!(
        out,
        "  ZR production raw saved:     {zr_saved:.6e} ({:.2}%)",
        100.0 * zr_saved / before
    )
    .unwrap();
    writeln!(
        out,
        "  combined ZR+FF raw saved:    {:.6e} ({:.2}%)",
        zr_saved + ff_raw,
        100.0 * (zr_saved + ff_raw) / before
    )
    .unwrap();
    writeln!(out).unwrap();

    // 长度审计。
    writeln!(out, "production FIXED_FIRST shortcut lengths:").unwrap();
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

    // baseline 碰撞类型审计(原来撞了谁;含条数 / 频率质量 / 有效收益)。
    let mut classes: std::collections::BTreeMap<&str, (usize, u64, f64)> =
        std::collections::BTreeMap::new();
    for entry in &selection.selected {
        let slot = classes
            .entry(entry.baseline_collision_class.label())
            .or_default();
        slot.0 += 1;
        slot.1 += entry.frequency_score;
        slot.2 += entry.net_utility;
    }
    writeln!(out, "baseline collision classes(selected):").unwrap();
    for (label, (count, mass, utility)) in &classes {
        writeln!(
            out,
            "  {label}: {count} 条(frequency mass {mass}, effective benefit {utility:.6e})"
        )
        .unwrap();
    }
    writeln!(out).unwrap();

    // fanout / expected rank 分布(逐值计数 + nearest-rank 分位数)。
    // 无 fanout 上限:按真实出现值统计,并单独报告深 rank 规模。
    let mut fanouts: Vec<usize> = selection
        .selected
        .iter()
        .map(|e| e.baseline_fanout)
        .collect();
    fanouts.sort_unstable();
    let mut ranks: Vec<usize> = selection.selected.iter().map(|e| e.expected_rank).collect();
    ranks.sort_unstable();
    let max_fanout = fanouts.last().copied().unwrap_or(0);
    let max_rank = ranks.last().copied().unwrap_or(0);
    writeln!(out, "baseline fanout distribution(selected):").unwrap();
    write!(out, "  fanout:").unwrap();
    for fanout in 1..=max_fanout {
        let count = fanouts.iter().filter(|&&f| f == fanout).count();
        if count > 0 {
            write!(out, " {fanout}→{count}").unwrap();
        }
    }
    writeln!(out).unwrap();
    writeln!(
        out,
        "  P50={} P90={} P95={} P99={} max={}",
        percentile_nearest_rank(&fanouts, 50),
        percentile_nearest_rank(&fanouts, 90),
        percentile_nearest_rank(&fanouts, 95),
        percentile_nearest_rank(&fanouts, 99),
        max_fanout,
    )
    .unwrap();
    writeln!(
        out,
        "  expected rank: P50={} P90={} P95={} P99={} max={}",
        percentile_nearest_rank(&ranks, 50),
        percentile_nearest_rank(&ranks, 90),
        percentile_nearest_rank(&ranks, 95),
        percentile_nearest_rank(&ranks, 99),
        max_rank,
    )
    .unwrap();
    let rank_ge_10 = ranks.iter().filter(|&&r| r >= 10).count();
    let rank_ge_16 = ranks.iter().filter(|&&r| r >= 16).count();
    writeln!(
        out,
        "  deep-rank selected: rank>=10 有 {rank_ge_10} 条,rank>=16 有 {rank_ge_16} 条,max rank={max_rank}"
    )
    .unwrap();
    writeln!(out).unwrap();

    // top 50 production(词频降序,词升序兜底)。
    let mut by_frequency = selection.selected.iter().collect::<Vec<_>>();
    by_frequency.sort_by(|a, b| {
        b.frequency_score
            .cmp(&a.frequency_score)
            .then(a.word.cmp(&b.word))
    });
    writeln!(out, "top 50 production FIXED_FIRST(by frequency):").unwrap();
    for (index, entry) in by_frequency.iter().take(50).enumerate() {
        writeln!(
            out,
            "  {:2}. {} {} {} {} score={} fanout={} rank={} utility={:.4} stability={}/{} class={}",
            index + 1,
            entry.word,
            entry.full_code,
            entry.shortcut_code,
            entry.mode,
            entry.frequency_score,
            entry.baseline_fanout,
            entry.expected_rank,
            entry.net_utility,
            entry.top_code_votes,
            entry.total_runs,
            entry.baseline_collision_class.label(),
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
            "  {:2}. {} ref={} score={} fanout={} stability={}/{} top_code={} reason={:?}",
            index + 1,
            exclusion.word,
            exclusion.reference_code,
            exclusion.frequency_score,
            exclusion.baseline_fanout,
            exclusion.votes.0,
            exclusion.votes.1,
            exclusion.top_code.as_deref().unwrap_or("-"),
            exclusion.reason,
        )
        .unwrap();
    }
    writeln!(out).unwrap();

    // top 30 deepest selected(deep-rank 审计;fanout 无上限,如实报告深度)。
    let mut by_fanout = selection.selected.iter().collect::<Vec<_>>();
    by_fanout.sort_by(|a, b| {
        b.baseline_fanout
            .cmp(&a.baseline_fanout)
            .then(b.frequency_score.cmp(&a.frequency_score))
            .then(a.word.cmp(&b.word))
    });
    writeln!(out, "top 30 deepest selected(deep-rank 审计):").unwrap();
    for (index, entry) in by_fanout.iter().take(30).enumerate() {
        writeln!(
            out,
            "  {:2}. {} {} fanout={} rank={} score={} utility={:.4} stability={}/{}",
            index + 1,
            entry.word,
            entry.shortcut_code,
            entry.baseline_fanout,
            entry.expected_rank,
            entry.frequency_score,
            entry.net_utility,
            entry.top_code_votes,
            entry.total_runs,
        )
        .unwrap();
    }
    writeln!(out).unwrap();

    // prefix 拓扑审计(全量静态;runtime 哨兵供 tests/librime 使用)。
    let prefix_audit = xhup_analyzer::audit_fixed_first_prefix_topology(
        &CodeOccupancy::build_current_production(),
    );
    writeln!(out, "FIXED_FIRST prefix 拓扑审计(全量静态):").unwrap();
    writeln!(
        out,
        "  production FIXED_FIRST shortcuts: {}",
        prefix_audit.shortcut_count
    )
    .unwrap();
    writeln!(
        out,
        "  FF shortcut 是更长合法码 strict prefix 的对数: {}",
        prefix_audit.shortcut_prefix_of_longer_pairs
    )
    .unwrap();
    writeln!(
        out,
        "  其中 distinct FF shortcut 数:                {}",
        prefix_audit.shortcuts_prefixing_longer
    )
    .unwrap();
    writeln!(
        out,
        "  更短合法码是 FF shortcut strict prefix 的对数: {}",
        prefix_audit.shorter_prefix_of_shortcut_pairs
    )
    .unwrap();
    writeln!(
        out,
        "  FF shortcut 互为 strict prefix 的对数:         {}",
        prefix_audit.shortcut_to_shortcut_pairs
    )
    .unwrap();
    if let Some((shorter, shortcut, word)) = &prefix_audit.reverse_example {
        writeln!(
            out,
            "  reverse prefix runtime 代表: {shorter} → {shortcut}({word})"
        )
        .unwrap();
    }
    writeln!(out).unwrap();
    writeln!(out, "各码长 runtime continuation 哨兵(deterministic):").unwrap();
    for length in &prefix_audit.lengths {
        if length.rows == 0 {
            writeln!(out, "  {}-key: 0 条(N/A)", length.length).unwrap();
            continue;
        }
        match &length.continuation {
            Some(s) => writeln!(
                out,
                "  {}-key: {} 条  continuation: {} {} {} {} → 继续到 {}({})",
                length.length,
                length.rows,
                s.word,
                s.full_code,
                s.shortcut_code,
                s.mode,
                s.longer_code,
                s.longer_target,
            )
            .unwrap(),
            None => writeln!(
                out,
                "  {}-key: {} 条  continuation: N/A(无 strict-prefix 关系)",
                length.length, length.rows
            )
            .unwrap(),
        }
    }
    writeln!(out).unwrap();

    // 「时间」哨兵(full uijm):本 PR 的核心 regression anchor。
    // 若 incremental policy 没有重新得出 时间 -> uij,这是 STOP 条件,
    // 绝不允许人工 whitelist。
    writeln!(out, "「时间」哨兵(full uijm):").unwrap();
    let Some(time_target) = data.targets.iter().find(|t| t.word() == "时间") else {
        writeln!(out, "  STOP: 词目标中不存在「时间」").unwrap();
        return out;
    };
    writeln!(out, "  frequency score: {}", time_target.frequency_score()).unwrap();
    if let Some(full_group) = data.occupancy.group(time_target.full_code()) {
        let menu: Vec<&str> = full_group.iter().map(|c| c.text()).collect();
        let rank = full_group
            .iter()
            .find(|c| c.text() == "时间")
            .map_or(0, |c| c.rank());
        writeln!(
            out,
            "  full {}: baseline fanout={} rank={} menu={}",
            time_target.full_code(),
            menu.len(),
            rank,
            menu.join(", ")
        )
        .unwrap();
    }
    let evaluations = xhup_analyzer::evaluate_target(
        time_target,
        &data.occupancy,
        &data.frequency,
        &xhup_analyzer::reference_scale(),
        &xhup_analyzer::production::reference_cost(),
        OptimizationProfile::FixedFirst,
    );
    let robustness = evidence.robustness.get("时间");
    for code in ["uij", "ujm"] {
        let key: KeySequence = code.parse().expect("哨兵码合法");
        let menu: Vec<&str> = data
            .occupancy
            .group(&key)
            .map(|group| group.iter().map(|c| c.text()).collect())
            .unwrap_or_default();
        let evaluation = evaluations.iter().find(|e| e.shortcut_code == key);
        let votes = robustness
            .and_then(|r| r.code_votes.get(code))
            .copied()
            .unwrap_or(0);
        let total_runs = robustness.map_or(0, |r| r.total_runs);
        let selected = selection
            .selected
            .iter()
            .find(|e| e.word == "时间" && e.shortcut_code == key);
        writeln!(out, "  {code}:").unwrap();
        writeln!(out, "    baseline fanout: {}", menu.len()).unwrap();
        writeln!(out, "    baseline menu:   {}", menu.join(", ")).unwrap();
        if let Some(evaluation) = evaluation {
            writeln!(out, "    mode:            {}", evaluation.mode).unwrap();
            writeln!(
                out,
                "    expected rank:   {}",
                evaluation.existing_fanout + 1
            )
            .unwrap();
            writeln!(
                out,
                "    reference net utility: {:.4}",
                evaluation.breakdown.net_utility
            )
            .unwrap();
            writeln!(
                out,
                "    eligible: {}{}",
                evaluation.eligible,
                evaluation
                    .gate_reason
                    .map_or(String::new(), |r| format!("(gate: {r})"))
            )
            .unwrap();
        }
        writeln!(out, "    same-code votes: {votes}/{total_runs}").unwrap();
        match selected {
            Some(entry) => writeln!(
                out,
                "    production:      SELECTED({} {} {} {})",
                entry.word, entry.full_code, entry.shortcut_code, entry.mode
            )
            .unwrap(),
            None => writeln!(out, "    production:      not selected").unwrap(),
        }
    }
    let time_selected = selection
        .selected
        .iter()
        .any(|e| e.word == "时间" && e.shortcut_code.to_string() == "uij");
    writeln!(
        out,
        "  sentinel verdict: 时间 -> uij {}",
        if time_selected {
            "SELECTED(符合预期)"
        } else {
            "NOT SELECTED(STOP 条件,禁止人工 whitelist)"
        }
    )
    .unwrap();
    out
}
