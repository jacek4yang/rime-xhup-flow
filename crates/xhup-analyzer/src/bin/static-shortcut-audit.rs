//! `static-shortcut-audit`:静态简码体系审计 + 二码研究 + 二码零冲突
//! 生产导出的命令行工具。
//!
//! 纯分析与导出编排:研究部分不修改任何 production 产物;production
//! 导出(`--dump-production-two-key-zero-regression`)输出 canonical TSV,
//! 入库需 diff review 与 policy review。

use std::collections::BTreeMap;
use std::process::ExitCode;
use std::time::Instant;

use xhup_analyzer::frequency::{CharCodeUsage, FrequencyScale};
use xhup_analyzer::production_two_key;
use xhup_analyzer::two_key_study::{self, TwoKeyPlacement, TwoKeyUniverse, run_two_key_grid};
use xhup_analyzer::{AnalysisData, build_analysis_with_spec};

fn usage() -> ! {
    eprintln!(
        "用法: static-shortcut-audit [选项]\n\
         \n\
         选项:\n\
         \x20 --study-two-key          二码词语简码研究(II 候选 / 当前最优路径 /\n\
         \x20                           SAFE_APPEND / OPTIMAL_INSERT / 敏感性网格)\n\
         \x20 --dump-candidates <path> 二码研究候选 TSV(全部 II 候选;研究产物,\n\
         \x20                           不入库)\n\
         \x20 --dump-production-two-key-zero-regression <path>\n\
         \x20                           导出二码零冲突生产 canonical TSV\n\
         \x20                           (policy two-key-zero-regression-v1;仅空码;\n\
         \x20                           导出后退出)\n\
         \x20 --dump-two-key-audit-manifest <path>\n\
         \x20                           导出二码 runtime 审计 manifest\n\
         \x20                           (全部占用 2 键码的既有菜单 + 全部选定映射;\n\
         \x20                           供 tests/librime 的 C 审计使用)\n\
         \x20 --static-report          静态多级简码体系全量报告(码长 × 来源层)\n\
         \n\
         研究产物请输出到临时路径,不要 commit。production 导出是 canonical\n\
         生产数据:入库需 diff review 与 policy review。"
    );
    std::process::exit(2);
}

fn main() -> ExitCode {
    let mut study_two_key = false;
    let mut dump_candidates_path: Option<String> = None;
    let mut dump_production_two_key_path: Option<String> = None;
    let mut dump_two_key_manifest_path: Option<String> = None;
    let mut static_report = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--study-two-key" => study_two_key = true,
            "--dump-candidates" => {
                dump_candidates_path = Some(args.next().unwrap_or_else(|| usage()));
            }
            "--dump-production-two-key-zero-regression" => {
                dump_production_two_key_path = Some(args.next().unwrap_or_else(|| usage()));
            }
            "--dump-two-key-audit-manifest" => {
                dump_two_key_manifest_path = Some(args.next().unwrap_or_else(|| usage()));
            }
            "--static-report" => static_report = true,
            "--help" | "-h" => usage(),
            _ => usage(),
        }
    }
    if !study_two_key
        && dump_candidates_path.is_none()
        && dump_production_two_key_path.is_none()
        && dump_two_key_manifest_path.is_none()
        && !static_report
    {
        usage();
    }

    if static_report {
        print!("{}", render_static_report());
    }

    // ── 二码研究 + 生产导出(共享一次网格) ──
    if study_two_key
        || dump_candidates_path.is_some()
        || dump_production_two_key_path.is_some()
        || dump_two_key_manifest_path.is_some()
    {
        let start = Instant::now();
        let universe = TwoKeyUniverse::build();
        let build_elapsed = start.elapsed();
        eprintln!(
            "二码研究全集:{} 个 2 字词目标 / {} 个 II 候选({:.3}s)",
            universe.two_char_target_count,
            universe.candidates.len(),
            build_elapsed.as_secs_f64()
        );

        let start = Instant::now();
        let runs = run_two_key_grid(&universe);
        let grid_elapsed = start.elapsed();
        eprintln!(
            "敏感性网格:{} 次运行({:.3}s)",
            runs.len(),
            grid_elapsed.as_secs_f64()
        );

        // 二码 runtime 审计 manifest:全部占用 2 键码的既有菜单
        //(P0:菜单 before == after)+ 全部选定映射(空码 rank 1)。
        if let Some(path) = dump_two_key_manifest_path {
            let (selected, _) = production_two_key::select_two_key_production(&universe, &runs);
            let manifest = two_key_audit_manifest(&universe, &selected);
            if let Err(error) = std::fs::write(&path, manifest) {
                eprintln!("写入 {path} 失败:{error}");
                return ExitCode::FAILURE;
            }
            let occupied_count = universe
                .candidates
                .iter()
                .filter(|c| c.code_class == two_key_study::TwoKeyCodeClass::OccupiedByChars)
                .count();
            eprintln!(
                "二码 runtime 审计 manifest → {path}({} 占用码 + {} 选定映射)",
                occupied_count,
                selected.len()
            );
            return ExitCode::SUCCESS;
        }

        // production 导出快速路径。
        if let Some(path) = dump_production_two_key_path {
            let (selected, audit) = production_two_key::select_two_key_production(&universe, &runs);
            let benefit = production_two_key::two_key_benefit_audit(&universe, &selected);
            let tsv = production_two_key::serialize_two_key_tsv(&selected);
            if let Err(error) = std::fs::write(&path, tsv) {
                eprintln!("写入 {path} 失败:{error}");
                return ExitCode::FAILURE;
            }
            eprint!(
                "{}",
                render_two_key_production_audit(&path, &universe, &selected, &audit, &benefit)
            );
            // 生产导出不需要完整研究报告。
            return ExitCode::SUCCESS;
        }

        if study_two_key {
            print!(
                "{}",
                render_two_key_study_report(&universe, &runs, build_elapsed, grid_elapsed)
            );
        }
        if let Some(path) = dump_candidates_path {
            let tsv = dump_candidates_tsv(&universe, &runs);
            if let Err(error) = std::fs::write(&path, tsv) {
                eprintln!("写入 {path} 失败:{error}");
                return ExitCode::FAILURE;
            }
            eprintln!(
                "二码研究候选 TSV → {path}({} 行)",
                universe.candidates.len()
            );
        }
    }
    ExitCode::SUCCESS
}

// ── 研究报告 ─────────────────────────────────────────────────

fn fmt_seconds(d: std::time::Duration) -> String {
    format!("{:.3}s", d.as_secs_f64())
}

/// nearest-rank 分位数(空集返回 0)。
fn percentile_nearest_rank(sorted: &[usize], p: usize) -> usize {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (p * sorted.len()).div_ceil(100);
    sorted[rank.max(1) - 1]
}

fn render_two_key_study_report(
    universe: &TwoKeyUniverse,
    runs: &[two_key_study::TwoKeyStudyRun],
    build_elapsed: std::time::Duration,
    grid_elapsed: std::time::Duration,
) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    let reference = two_key_study::reference_run_index(runs);
    writeln!(
        out,
        "# 二码词语简码研究({};grammar {})",
        two_key_study::TWO_KEY_STUDY_VERSION,
        xhup_analyzer::candidates::CandidateGrammar::MonotoneSuffixInitialsV2.label()
    )
    .unwrap();
    writeln!(out, "研究语义,非 production 推荐;占用码不生产化。").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "timings: build {} / grid {}",
        fmt_seconds(build_elapsed),
        fmt_seconds(grid_elapsed)
    )
    .unwrap();
    writeln!(out).unwrap();

    // ── 全集与码空间审计 ──
    let occupied: Vec<&two_key_study::TwoKeyCandidate> = universe
        .candidates
        .iter()
        .filter(|c| c.code_class == two_key_study::TwoKeyCodeClass::OccupiedByChars)
        .collect();
    let empty: Vec<&two_key_study::TwoKeyCandidate> = universe
        .candidates
        .iter()
        .filter(|c| c.code_class == two_key_study::TwoKeyCodeClass::Empty)
        .collect();
    let distinct_codes: std::collections::BTreeSet<_> = universe
        .candidates
        .iter()
        .map(|c| c.two_key_code.to_string())
        .collect();
    let distinct_occupied: std::collections::BTreeSet<_> = occupied
        .iter()
        .map(|c| c.two_key_code.to_string())
        .collect();
    let distinct_empty: std::collections::BTreeSet<_> =
        empty.iter().map(|c| c.two_key_code.to_string()).collect();
    writeln!(out, "## 全集与码空间").unwrap();
    writeln!(
        out,
        "  2 字词目标: {} / II 候选: {}",
        universe.two_char_target_count,
        universe.candidates.len()
    )
    .unwrap();
    writeln!(
        out,
        "  distinct II 码: {}(占用 {} / 空闲 {})",
        distinct_codes.len(),
        distinct_occupied.len(),
        distinct_empty.len()
    )
    .unwrap();
    writeln!(
        out,
        "  2 键单字关系: {} / distinct 占用单字码: {}",
        universe.char_domain.relation_count(),
        distinct_occupied.len()
    )
    .unwrap();
    writeln!(out).unwrap();

    // 2 键单字 fanout 分布。
    let mut fanouts: Vec<usize> = distinct_occupied
        .iter()
        .map(|code| {
            let seq: xhup_core::KeySequence = code.parse().unwrap();
            universe.occupancy.fanout(&seq)
        })
        .collect();
    fanouts.sort_unstable();
    writeln!(out, "  2 键单字 fanout 分布(占用码):").unwrap();
    writeln!(
        out,
        "    P50={} P90={} P95={} P99={} max={}",
        percentile_nearest_rank(&fanouts, 50),
        percentile_nearest_rank(&fanouts, 90),
        percentile_nearest_rank(&fanouts, 95),
        percentile_nearest_rank(&fanouts, 99),
        fanouts.last().copied().unwrap_or(0)
    )
    .unwrap();
    // 每码词候选数分布。
    let mut per_code_counts: Vec<usize> = distinct_codes
        .iter()
        .map(|code| {
            universe
                .candidates
                .iter()
                .filter(|c| &c.two_key_code.to_string() == code)
                .count()
        })
        .collect();
    per_code_counts.sort_unstable();
    writeln!(out, "  每码 II 词候选数分布:").unwrap();
    writeln!(
        out,
        "    P50={} P90={} P95={} P99={} max={}",
        percentile_nearest_rank(&per_code_counts, 50),
        percentile_nearest_rank(&per_code_counts, 90),
        percentile_nearest_rank(&per_code_counts, 95),
        percentile_nearest_rank(&per_code_counts, 99),
        per_code_counts.last().copied().unwrap_or(0)
    )
    .unwrap();
    writeln!(out).unwrap();

    // ── 既有简码状态 / 当前最优路径分布 ──
    writeln!(out, "## 既有简码状态与当前最优路径(Universe B = 全部)").unwrap();
    let mut status_counts: std::collections::BTreeMap<&str, usize> =
        std::collections::BTreeMap::new();
    for candidate in &universe.candidates {
        let key = match candidate.existing_shortcut {
            two_key_study::ExistingShortcutStatus::None => "NO_SHORTCUT",
            two_key_study::ExistingShortcutStatus::ZeroRegression { .. } => "ZR",
            two_key_study::ExistingShortcutStatus::FixedFirst { .. } => "FIXED_FIRST",
        };
        *status_counts.entry(key).or_default() += 1;
    }
    for (key, count) in &status_counts {
        writeln!(out, "  {key}: {count}").unwrap();
    }
    let mut route_counts: std::collections::BTreeMap<&str, usize> =
        std::collections::BTreeMap::new();
    let mut route_costs: std::collections::BTreeMap<&str, Vec<f64>> =
        std::collections::BTreeMap::new();
    for candidate in &universe.candidates {
        *route_counts
            .entry(candidate.current_best.kind.label())
            .or_default() += 1;
        route_costs
            .entry(candidate.current_best.kind.label())
            .or_default()
            .push(candidate.current_best.cost.total());
    }
    for kind in ["FULL_CODE", "ZR_SHORTCUT", "FIXED_FIRST_SHORTCUT"] {
        let count = route_counts.get(kind).copied().unwrap_or(0);
        let mut costs = route_costs.get(kind).cloned().unwrap_or_default();
        costs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p = |q: usize| {
            if costs.is_empty() {
                0.0
            } else {
                costs[(q * costs.len()).div_ceil(100).max(1) - 1]
            }
        };
        let mean = costs.iter().sum::<f64>() / count.max(1) as f64;
        writeln!(
            out,
            "  best={kind}: {count} 词(mean {mean:.3} / P50 {:.3} / P90 {:.3} / P95 {:.3} / P99 {:.3})",
            p(50),
            p(90),
            p(95),
            p(99)
        )
        .unwrap();
    }
    writeln!(out).unwrap();

    // ── SAFE_APPEND / OPTIMAL_INSERT(Universe A 与 B 分别) ──
    for universe_label in [
        "Universe A(无既有简码,production 兼容)",
        "Universe B(全部,研究上限)",
    ] {
        let in_universe = |c: &two_key_study::TwoKeyCandidate| match universe_label {
            s if s.starts_with("Universe A") => c.existing_shortcut.is_none(),
            _ => true,
        };
        let indices: Vec<usize> = universe
            .candidates
            .iter()
            .enumerate()
            .filter(|(_, c)| in_universe(c))
            .map(|(i, _)| i)
            .collect();
        writeln!(out, "## {universe_label}").unwrap();
        writeln!(out, "  targets: {}", indices.len()).unwrap();
        let mut safe_positive = 0usize;
        let mut safe_zero = 0usize;
        let mut safe_negative = 0usize;
        let mut safe_raw_gain: f64 = 0.0;
        let mut safe_effective_gain: f64 = 0.0;
        let mut safe_ranks: Vec<usize> = Vec::new();
        for &index in &indices {
            let evaluation = &runs[reference].safe[index];
            if evaluation.weighted_net_gain > 0.0 {
                safe_positive += 1;
                safe_effective_gain += evaluation.weighted_net_gain;
                let candidate = &universe.candidates[index];
                safe_raw_gain += (candidate.current_best.cost.typed_keys - 2.0)
                    * universe.frequency.target_weight(
                        &production_two_key::reference_scale(),
                        candidate.frequency_score,
                    );
            } else if evaluation.weighted_net_gain < 0.0 {
                safe_negative += 1;
            } else {
                safe_zero += 1;
            }
            safe_ranks.push(evaluation.rank as usize);
        }
        safe_ranks.sort_unstable();
        writeln!(
            out,
            "  SAFE_APPEND: positive {safe_positive} / negative {safe_negative} / zero {safe_zero}"
        )
        .unwrap();
        writeln!(
            out,
            "    safe rank: P50={} P90={} P95={} P99={} max={}",
            percentile_nearest_rank(&safe_ranks, 50),
            percentile_nearest_rank(&safe_ranks, 90),
            percentile_nearest_rank(&safe_ranks, 95),
            percentile_nearest_rank(&safe_ranks, 99),
            safe_ranks.last().copied().unwrap_or(0)
        )
        .unwrap();
        writeln!(
            out,
            "    weighted raw gain(正候选): {safe_raw_gain:.6e} / effective gain: {safe_effective_gain:.6e}"
        )
        .unwrap();

        // OPTIMAL_INSERT。
        let mut optimal_positive = 0usize;
        let mut optimal_word_benefit: f64 = 0.0;
        let mut optimal_disruption: f64 = 0.0;
        let mut optimal_net: f64 = 0.0;
        let mut rank1 = 0usize;
        let mut middle = 0usize;
        let mut append = 0usize;
        let mut top1_changes = 0usize;
        let mut rank_changes = 0usize;
        for &index in &indices {
            let evaluation = &runs[reference].optimal[index];
            if evaluation.weighted_net_gain > 0.0 {
                optimal_positive += 1;
                optimal_word_benefit +=
                    evaluation.weighted_net_gain + evaluation.weighted_char_disruption;
                optimal_disruption += evaluation.weighted_char_disruption;
                optimal_net += evaluation.weighted_net_gain;
                match evaluation.rank {
                    1 => rank1 += 1,
                    r if r as usize == evaluation.projected_fanout => append += 1,
                    _ => middle += 1,
                }
                if evaluation.rank == 1 {
                    top1_changes += 1;
                }
                rank_changes += evaluation.displaced_char_count;
            }
        }
        writeln!(
            out,
            "  OPTIMAL_INSERT: positive {optimal_positive}(rank1 {rank1} / middle {middle} / append {append})"
        )
        .unwrap();
        writeln!(
            out,
            "    weighted word benefit: {optimal_word_benefit:.6e} / char disruption: {optimal_disruption:.6e} / net: {optimal_net:.6e}",
        )
        .unwrap();
        writeln!(
            out,
            "    implied top1 changes: {top1_changes} / rank changes: {rank_changes}"
        )
        .unwrap();
        writeln!(out).unwrap();
    }

    // ── 「时间」哨兵 ──
    writeln!(out, "## 「时间」哨兵(研究,符号不预设)").unwrap();
    if let Some(index) = universe.candidates.iter().position(|c| c.word == "时间") {
        let candidate = &universe.candidates[index];
        writeln!(
            out,
            "  full {} / II {} / 既有简码 uij(FI)",
            candidate.full_code, candidate.two_key_code
        )
        .unwrap();
        writeln!(
            out,
            "  当前最优路径: {} {} rank={} cost={:.3}",
            candidate.current_best.kind.label(),
            candidate.current_best.code,
            candidate.current_best.rank,
            candidate.current_best.cost.total()
        )
        .unwrap();
        let menu: Vec<String> = universe
            .occupancy
            .group(&candidate.two_key_code)
            .map(|g| g.iter().map(|c| c.text().to_string()).collect())
            .unwrap_or_default();
        writeln!(
            out,
            "  uj 2 键单字 fanout: {} menu(前 10): {}",
            candidate.char_fanout,
            menu.iter().take(10).cloned().collect::<Vec<_>>().join(",")
        )
        .unwrap();
        let safe = &runs[reference].safe[index];
        writeln!(
            out,
            "  SAFE_APPEND: rank {} cost {:.3} net gain {:.6e}({})",
            safe.rank,
            safe.cost.total(),
            safe.weighted_net_gain,
            if safe.weighted_net_gain > 0.0 {
                "positive"
            } else {
                "NOT positive"
            }
        )
        .unwrap();
        let optimal = &runs[reference].optimal[index];
        writeln!(
            out,
            "  OPTIMAL_INSERT: rank {} cost {:.3} word benefit {:.6e} char disruption {:.6e} net {:.6e} displaced {}",
            optimal.rank,
            optimal.cost.total(),
            optimal.weighted_net_gain + optimal.weighted_char_disruption,
            optimal.weighted_char_disruption,
            optimal.weighted_net_gain,
            optimal.displaced_chars.len()
        )
        .unwrap();
        writeln!(
            out,
            "  production: {}(占用码,不由 two-key-zero-regression 生产化)",
            if candidate.char_fanout == 0 {
                "可能(空码)"
            } else {
                "NO"
            }
        )
        .unwrap();
    }
    writeln!(out).unwrap();

    // ── 空 2 键码机会 ──
    writeln!(out, "## 空 2 键码机会(EMPTY_2KEY_CODE)").unwrap();
    let empty_indices: Vec<usize> = universe
        .candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| c.code_class == two_key_study::TwoKeyCodeClass::Empty)
        .map(|(i, _)| i)
        .collect();
    writeln!(
        out,
        "  空码候选词: {} / distinct 空码: {}",
        empty_indices.len(),
        distinct_empty.len()
    )
    .unwrap();
    let scale = production_two_key::reference_scale();
    let mut empty_gain: f64 = 0.0;
    for &index in &empty_indices {
        empty_gain += runs[reference].safe[index].weighted_net_gain;
    }
    writeln!(out, "  空码候选合计净收益(reference): {empty_gain:.6e}").unwrap();
    // top 20 空码机会(净收益降序)。
    let mut top_empty: Vec<usize> = empty_indices
        .iter()
        .copied()
        .filter(|&i| universe.candidates[i].existing_shortcut.is_none())
        .collect();
    top_empty.sort_by(|&a, &b| {
        let gain_a = runs[reference].safe[a].weighted_net_gain;
        let gain_b = runs[reference].safe[b].weighted_net_gain;
        gain_b.partial_cmp(&gain_a).unwrap().then(
            universe.candidates[a]
                .word
                .cmp(&universe.candidates[b].word),
        )
    });
    writeln!(out, "  top 20 空码机会(Universe A,净收益降序):").unwrap();
    for index in top_empty.iter().take(20) {
        let candidate = &universe.candidates[*index];
        let evaluation = &runs[reference].safe[*index];
        writeln!(
            out,
            "    {} {} {} score={} current={}(cost {:.3}) 2key cost {:.3} gain {:.6e}",
            candidate.word,
            candidate.full_code,
            candidate.two_key_code,
            candidate.frequency_score,
            candidate.current_best.kind.label(),
            candidate.current_best.cost.total(),
            evaluation.cost.total(),
            evaluation.weighted_net_gain
        )
        .unwrap();
    }
    let _ = scale;
    writeln!(out).unwrap();

    // ── 与 3 键静态层收益对比(同 100k 分母) ──
    writeln!(
        out,
        "## 静态层收益对比(reference 尺度,denominator = 全部 100k 词目标)"
    )
    .unwrap();
    let data: AnalysisData = build_analysis_with_spec(
        xhup_analyzer::candidates::CandidateEnumerationSpec::LEGACY_V1_FROZEN,
    );
    let before: f64 = data
        .targets
        .iter()
        .map(|t| {
            data.frequency
                .target_weight(&production_two_key::reference_scale(), t.frequency_score())
                * t.full_code().len() as f64
        })
        .sum();
    let zr_saved = zr_raw_saving(&data);
    let ff_saved = ff_raw_saving(&data);
    let two_key_raw: f64 = empty_indices
        .iter()
        .filter(|&&i| universe.candidates[i].existing_shortcut.is_none())
        .map(|&i| {
            let c = &universe.candidates[i];
            (c.current_best.cost.typed_keys - 2.0)
                * universe
                    .frequency
                    .target_weight(&production_two_key::reference_scale(), c.frequency_score)
        })
        .sum();
    writeln!(out, "  weighted keys before:    {before:.6e}").unwrap();
    writeln!(
        out,
        "  ZR raw saving:           {zr_saved:.6e} ({:.2}%)",
        100.0 * zr_saved / before
    )
    .unwrap();
    writeln!(
        out,
        "  FIXED_FIRST raw saving:  {ff_saved:.6e} ({:.2}%)",
        100.0 * ff_saved / before
    )
    .unwrap();
    writeln!(
        out,
        "  2-key 空码理论 raw(全量,非每码择优): {two_key_raw:.6e} ({:.2}%)",
        100.0 * two_key_raw / before
    )
    .unwrap();
    writeln!(
        out,
        "  combined ZR+FF raw:      {:.6e} ({:.2}%)",
        zr_saved + ff_saved,
        100.0 * (zr_saved + ff_saved) / before
    )
    .unwrap();
    writeln!(out).unwrap();

    // ── 决策矩阵 ──
    writeln!(out, "## 决策矩阵(事实,无代码内置推荐)").unwrap();
    writeln!(
        out,
        "  策略                     既有名次变化  额外收益上限            复杂度"
    )
    .unwrap();
    writeln!(
        out,
        "  EMPTY 2-key only         0             本报告空码节             低(与 ZR 同构)"
    )
    .unwrap();
    writeln!(
        out,
        "  SAFE_APPEND occupied     0             SAFE_APPEND 节          中(第二 translator 追加语义)"
    )
    .unwrap();
    writeln!(
        out,
        "  OPTIMAL_INSERT           重排 2 键单字  OPTIMAL_INSERT 节       高(破坏单字肌肉记忆,P0 风险)"
    )
    .unwrap();
    out
}

/// ZR 层 raw 加权节省(冻结 canonical 行现算)。
fn zr_raw_saving(data: &AnalysisData) -> f64 {
    let scale = production_two_key::reference_scale();
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
                .expect("ZR canonical (词, 完整码) 必然存在于词层");
            data.frequency.target_weight(&scale, score)
                * (entry.full_code().len() - entry.shortcut_code().len()) as f64
        })
        .sum()
}

/// FF 层 raw 加权节省。
fn ff_raw_saving(data: &AnalysisData) -> f64 {
    let scale = production_two_key::reference_scale();
    let scores: std::collections::BTreeMap<(&str, &xhup_core::KeySequence), u64> = data
        .words
        .iter()
        .map(|entry| ((entry.word(), entry.code()), entry.frequency_score()))
        .collect();
    xhup_generator::canonical_fixed_first_shortcut_entries()
        .iter()
        .map(|entry| {
            let score = scores
                .get(&(entry.word(), entry.full_code()))
                .copied()
                .expect("FF canonical (词, 完整码) 必然存在于词层");
            data.frequency.target_weight(&scale, score)
                * (entry.full_code().len() - entry.shortcut_code().len()) as f64
        })
        .sum()
}

/// 研究候选 TSV 转储(§27 字段;确定性排序 = 词升序)。
fn dump_candidates_tsv(
    universe: &TwoKeyUniverse,
    runs: &[two_key_study::TwoKeyStudyRun],
) -> String {
    use std::fmt::Write as _;
    let reference = two_key_study::reference_run_index(runs);
    let mut out = String::new();
    writeln!(
        out,
        "word\tfull_code\ttwo_key_code\tmode\tfrequency_score\t\
         current_route_kind\tcurrent_route_code\tcurrent_route_rank\tcurrent_route_cost\t\
         char_fanout\tcode_class\t\
         safe_rank\tsafe_cost\tsafe_net_gain\t\
         optimal_rank\toptimal_cost\toptimal_char_disruption\toptimal_net_gain\t\
         existing_shortcut_kind\texisting_shortcut_code"
    )
    .unwrap();
    let mut rows: Vec<String> = Vec::with_capacity(universe.candidates.len());
    for (index, candidate) in universe.candidates.iter().enumerate() {
        let safe = &runs[reference].safe[index];
        let optimal = &runs[reference].optimal[index];
        let (existing_kind, existing_code) = match &candidate.existing_shortcut {
            two_key_study::ExistingShortcutStatus::None => ("NONE".to_string(), String::new()),
            two_key_study::ExistingShortcutStatus::ZeroRegression { code } => {
                ("ZR".to_string(), code.to_string())
            }
            two_key_study::ExistingShortcutStatus::FixedFirst { code } => {
                ("FIXED_FIRST".to_string(), code.to_string())
            }
        };
        let mut row = String::new();
        write!(
            row,
            "{}\t{}\t{}\tII\t{}\t{}\t{}\t{}\t{:.6}\t{}\t{}\t{}\t{:.6}\t{:.9e}\t{}\t{:.6}\t{:.9e}\t{:.9e}\t{}\t{}",
            candidate.word,
            candidate.full_code,
            candidate.two_key_code,
            candidate.frequency_score,
            candidate.current_best.kind.label(),
            candidate.current_best.code,
            candidate.current_best.rank,
            candidate.current_best.cost.total(),
            candidate.char_fanout,
            candidate.code_class.label(),
            safe.rank,
            safe.cost.total(),
            safe.weighted_net_gain,
            optimal.rank,
            optimal.cost.total(),
            optimal.weighted_char_disruption,
            optimal.weighted_net_gain + optimal.weighted_char_disruption,
            existing_kind,
            existing_code,
        )
        .unwrap();
        rows.push(row);
    }
    // 确定性排序:词升序(universe.candidates 已按词序)。
    for row in rows {
        out.push_str(&row);
        out.push('\n');
    }
    out
}

/// 二码 runtime 审计 manifest。
///
/// 两种行(`#` 头之后):
/// - `occupied\t<码>\t<fanout>\t<菜单(逗号分隔,名次序)>`:全部既有占用
///   2 键码的当前(变更前)菜单 —— C 审计断言 PRODUCTION 菜单与之完全
///   一致(P0:既有 2 键单字菜单逐项不变);
/// - `selected\t<码>\t<词>\t<完整码>`:全部选定二码映射 —— C 审计断言
///   该码菜单恰为该词一个候选且 rank 1(空码语义)。
fn two_key_audit_manifest(
    universe: &TwoKeyUniverse,
    selected: &[production_two_key::TwoKeySelection],
) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    writeln!(out, "# XHUP Flow two-key runtime audit manifest.").unwrap();
    writeln!(
        out,
        "# policy: {}",
        production_two_key::TWO_KEY_PRODUCTION_POLICY_VERSION
    )
    .unwrap();
    // 全部占用 2 键码(来自 baseline 2 键单字层,全量 —— 不限于被 II
    // 候选命中的码;菜单取名次序)。
    let mut occupied_codes: BTreeMap<String, usize> = BTreeMap::new();
    for (hanzi, code, _score, _weight) in universe.char_domain.entries() {
        let fanout = universe.occupancy.fanout(code);
        occupied_codes.entry(code.to_string()).or_insert(fanout);
        let _ = hanzi;
    }
    for (code, fanout) in &occupied_codes {
        let seq: xhup_core::KeySequence = code.parse().expect("码合法");
        let menu: Vec<&str> = universe
            .occupancy
            .group(&seq)
            .map(|g| g.iter().map(|c| c.text()).collect())
            .unwrap_or_default();
        debug_assert_eq!(menu.len(), *fanout);
        writeln!(out, "occupied\t{code}\t{fanout}\t{}", menu.join(",")).unwrap();
    }
    for entry in selected {
        writeln!(
            out,
            "selected\t{}\t{}\t{}",
            entry.shortcut_code, entry.word, entry.full_code
        )
        .unwrap();
    }
    out
}

/// 二码零冲突生产导出审计摘要。
fn render_two_key_production_audit(
    path: &str,
    universe: &TwoKeyUniverse,
    selected: &[production_two_key::TwoKeySelection],
    audit: &production_two_key::TwoKeySelectionAudit,
    benefit: &production_two_key::TwoKeyBenefitAudit,
) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    writeln!(
        out,
        "production 二码零冲突导出 → {path}(policy {};{} 行)",
        production_two_key::TWO_KEY_PRODUCTION_POLICY_VERSION,
        selected.len()
    )
    .unwrap();
    writeln!(
        out,
        "reference: {:?} × normalized(50:50, Conservative)",
        production_two_key::reference_point()
    )
    .unwrap();
    writeln!(
        out,
        "selection audit: 空码候选 {} / 竞争码 {} / selected {}",
        audit.empty_code_candidates, audit.contested_codes, audit.selected
    )
    .unwrap();
    for (reason, count) in &audit.excluded {
        writeln!(out, "  excluded {reason:?}: {count}").unwrap();
    }
    writeln!(out).unwrap();
    writeln!(out, "weighted benefit(reference 尺度):").unwrap();
    writeln!(out, "  raw keys saved:     {:.6e}", benefit.raw_keys_saved).unwrap();
    writeln!(
        out,
        "  effective benefit:  {:.6e}",
        benefit.effective_benefit
    )
    .unwrap();
    writeln!(out).unwrap();
    // top 30(净收益降序)。
    let mut by_gain = selected.to_vec();
    by_gain.sort_by(|a, b| {
        b.weighted_net_gain
            .partial_cmp(&a.weighted_net_gain)
            .unwrap()
            .then(a.word.cmp(&b.word))
    });
    writeln!(out, "top 30 production(净收益降序):").unwrap();
    for (index, entry) in by_gain.iter().take(30).enumerate() {
        writeln!(
            out,
            "  {:2}. {} {} {} score={} current={:.3} 2key={:.3} gain={:.6e} votes={}/{}",
            index + 1,
            entry.word,
            entry.full_code,
            entry.shortcut_code,
            entry.frequency_score,
            entry.current_best_cost,
            entry.two_key_cost,
            entry.weighted_net_gain,
            entry.top_word_votes,
            entry.total_runs
        )
        .unwrap();
    }
    let _ = universe;
    out
}

/// 静态多级简码体系全量报告(码长 × 来源层)。
fn render_static_report() -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    writeln!(out, "# 静态多级简码体系报告").unwrap();
    writeln!(out).unwrap();
    // policy 注册表。
    writeln!(out, "## policy 注册表").unwrap();
    for policy in xhup_analyzer::policy::ShortcutPolicyId::ALL {
        writeln!(
            out,
            "  {} (grammar: {}, frozen: {}, lengths {}..={})",
            policy.label(),
            policy
                .candidate_grammar()
                .map(|g| g.label())
                .unwrap_or("(无 F/I 语法)"),
            policy.is_frozen(),
            policy.shortcut_lengths().0,
            policy.shortcut_lengths().1
        )
        .unwrap();
    }
    writeln!(out).unwrap();
    // 元数据投影统计。
    let metadata = xhup_analyzer::policy::all_shortcut_metadata();
    let by_policy: std::collections::BTreeMap<String, usize> =
        metadata
            .iter()
            .fold(std::collections::BTreeMap::new(), |mut acc, m| {
                *acc.entry(m.policy.label().to_string()).or_default() += 1;
                acc
            });
    writeln!(out, "## 元数据投影(production 简码行)").unwrap();
    for (policy, count) in &by_policy {
        writeln!(out, "  {policy}: {count}").unwrap();
    }
    writeln!(out, "  total: {}", metadata.len()).unwrap();
    out
}

/// 避免 unused-import 警告的引用(报告内部使用)。
#[allow(unused)]
fn _touch() {
    let _ = TwoKeyPlacement::SafeAppend;
    let _ = FrequencyScale::Normalized {
        char_share: 0.5,
        usage: CharCodeUsage::Conservative,
    };
}
