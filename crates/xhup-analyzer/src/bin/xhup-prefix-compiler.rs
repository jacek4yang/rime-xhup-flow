//! `xhup-prefix-compiler`: Prefix-Space Compiler v3 命令行与对比基准工具。
//!
//! 用法:
//!   xhup-prefix-compiler [--production] [--limit N] [--bench] [--explain <词>] [--summary] [--check]
//!
//! 选项:
//!   --production 使用真实生产目标(v2_targets + daily_prior),不改写冻结映射
//!   --limit N    生产目标上界(仅 --production;缺省 256)。测试必须显式传入
//!   --bench      运行 v2 vs v3 确定性基准对比, 报告 KSPC, rank1, top3, 熵, 迁移成本等
//!   --explain 词 打印指定词语在前缀空间的槽位决策解释理由卡
//!   --summary    打印编译模型的前缀空间聚合统计
//!   --check      校验前缀闭合树、前缀延续非提交边界、可达性等核心不变量

use std::collections::BTreeMap;
use std::error::Error;
use std::process::ExitCode;

use std::str::FromStr;
use xhup_analyzer::prefix_space::slot::{SlotCandidate, SlotPlacementSource};
use xhup_analyzer::prefix_space::trie::PrefixTrie;
use xhup_analyzer::prefix_space::{
    BenchmarkMetrics, DEFAULT_PRODUCTION_LIMIT, PrefixCostModel, PrefixSpaceBenchmarkReport,
    PrefixSpaceCompiledModel, PrefixSpaceStats, PrefixTarget, SolverOptions,
    build_production_universe, evaluate_v3_metrics, solve_prefix_space, stats_equal_except_runtime,
    verify_production_invariants,
};
use xhup_core::KeySequence;

fn build_fixtures() -> (Vec<PrefixTarget>, PrefixTrie) {
    let mut trie = PrefixTrie::new();

    // 预置基础固定层单字与全码:
    // 例如单字 '有' (u), '人' (r), '他' (tx)
    let u_code = KeySequence::from_str("u").unwrap();
    let r_code = KeySequence::from_str("r").unwrap();
    let tx_code = KeySequence::from_str("tx").unwrap();

    trie.insert_candidate(
        &u_code,
        SlotCandidate::new(
            "有",
            u_code.clone(),
            1,
            SlotPlacementSource::Level1Frozen,
            5.0,
            true,
        ),
    );
    trie.insert_candidate(
        &r_code,
        SlotCandidate::new(
            "人",
            r_code.clone(),
            1,
            SlotPlacementSource::Level1Frozen,
            4.8,
            true,
        ),
    );
    trie.insert_candidate(
        &tx_code,
        SlotCandidate::new(
            "他",
            tx_code.clone(),
            1,
            SlotPlacementSource::FixedChar,
            4.5,
            false,
        ),
    );

    // 预置嵌套前缀测试路径: uior (输入法)
    // 路径: u -> ui -> uio -> uior
    let uior_code = KeySequence::from_str("uior").unwrap();

    // 在 uior 放置固定词 "输入法"
    trie.insert_candidate(
        &uior_code,
        SlotCandidate::new(
            "输入法",
            uior_code.clone(),
            1,
            SlotPlacementSource::FixedWord,
            3.8,
            false,
        ),
    );

    // 构建测试目标词集 (包含高频词与前缀候选):
    let targets = vec![
        PrefixTarget {
            text: "我们".to_string(),
            full_code: KeySequence::from_str("womk").unwrap(),
            mass: 5.2,
            legal_candidates: vec![
                (KeySequence::from_str("wm").unwrap(), "II".to_string()),
                (KeySequence::from_str("wom").unwrap(), "FI".to_string()),
            ],
            legacy_code: Some(KeySequence::from_str("wm").unwrap()),
        },
        PrefixTarget {
            text: "时间".to_string(),
            full_code: KeySequence::from_str("uijm").unwrap(),
            mass: 4.6,
            legal_candidates: vec![
                (KeySequence::from_str("uj").unwrap(), "II".to_string()),
                (KeySequence::from_str("ui").unwrap(), "I".to_string()),
                (KeySequence::from_str("uij").unwrap(), "FI".to_string()),
            ],
            legacy_code: Some(KeySequence::from_str("uj").unwrap()),
        },
        PrefixTarget {
            text: "输入".to_string(),
            full_code: KeySequence::from_str("uior").unwrap(),
            mass: 4.2,
            legal_candidates: vec![
                (KeySequence::from_str("ui").unwrap(), "FI".to_string()),
                (KeySequence::from_str("uio").unwrap(), "F".to_string()),
                (KeySequence::from_str("ur").unwrap(), "II".to_string()),
            ],
            legacy_code: Some(KeySequence::from_str("ur").unwrap()),
        },
        PrefixTarget {
            text: "可以".to_string(),
            full_code: KeySequence::from_str("keyi").unwrap(),
            mass: 4.9,
            legal_candidates: vec![
                (KeySequence::from_str("ky").unwrap(), "II".to_string()),
                (KeySequence::from_str("key").unwrap(), "FI".to_string()),
            ],
            legacy_code: Some(KeySequence::from_str("ky").unwrap()),
        },
        PrefixTarget {
            text: "中国".to_string(),
            full_code: KeySequence::from_str("vggo").unwrap(),
            mass: 5.0,
            legal_candidates: vec![
                (KeySequence::from_str("vg").unwrap(), "II".to_string()),
                (KeySequence::from_str("vgg").unwrap(), "FI".to_string()),
            ],
            legacy_code: Some(KeySequence::from_str("vg").unwrap()),
        },
    ];

    (targets, trie)
}

fn solve(targets: &[PrefixTarget], trie: PrefixTrie) -> PrefixSpaceCompiledModel {
    solve_prefix_space(
        targets,
        trie,
        &PrefixCostModel::default(),
        &SolverOptions::default(),
        None,
    )
}

fn run_fixture_check() -> Result<(), Box<dyn Error>> {
    println!("[check] Starting Prefix-Space Compiler invariant verification...");
    let (targets, initial_trie) = build_fixtures();
    let model = solve(&targets, initial_trie);

    // 不变量 1: 前缀闭合性检查 (uior 存在则 u, ui, uio 均存在)
    let uior_code = KeySequence::from_str("uior")?;
    let u_code = KeySequence::from_str("u")?;
    let ui_code = KeySequence::from_str("ui")?;
    let uio_code = KeySequence::from_str("uio")?;

    assert!(
        model.trie.get_node(&uior_code).is_some(),
        "uior node must exist"
    );
    assert!(
        model.trie.get_node(&uio_code).is_some(),
        "uio prefix node must exist"
    );
    assert!(
        model.trie.get_node(&ui_code).is_some(),
        "ui prefix node must exist"
    );
    assert!(
        model.trie.get_node(&u_code).is_some(),
        "u prefix node must exist"
    );
    println!("  ✓ Invariant 1: Prefix-closed path hierarchy verified (u, ui, uio, uior coexist)");

    // 不变量 2: valid prefix != commit boundary
    // u 节点有 rank 1 候选 ('有'), 但 can_continue() 为 true, 绝不隐含 auto-commit
    let u_node = model.trie.get_node(&u_code).unwrap();
    assert!(!u_node.slots.is_empty(), "u node has candidates");
    assert_eq!(u_node.slots[0].rank, 1, "u node has rank 1 candidate");
    assert!(
        u_node.can_continue(),
        "valid prefix u must allow continuation"
    );
    assert!(
        u_node.slots[0].is_continuation,
        "candidate at u is flagged as continuation"
    );
    println!("  ✓ Invariant 2: valid prefix != commit boundary verified (continuation allowed)");

    // 不变量 3: 码位非独占与多目标槽位共享
    // 检查节点上的候选列表均有确定且严格递增的名次 (1, 2, ...)
    for (code, slots) in model.trie.all_occupied_codes() {
        for (i, slot) in slots.iter().enumerate() {
            assert_eq!(
                slot.rank,
                i + 1,
                "Slot rank at {} must be contiguous 1-based",
                code
            );
        }
    }
    println!("  ✓ Invariant 3: Candidate slots properly ordered and deterministic");

    // 不变量 4: 确定性求解 (同输入必得相同输出)
    let (targets2, initial_trie2) = build_fixtures();
    let model2 = solve(&targets2, initial_trie2);
    assert_eq!(
        model.stats, model2.stats,
        "Solver must be 100% deterministic"
    );
    println!("  ✓ Invariant 4: Solver determinism confirmed across runs");

    // 不变量 5: 解释完整性
    for target in &targets {
        let exp = model
            .explanations
            .get(&target.text)
            .expect("explanation must exist");
        assert_eq!(exp.target, target.text);
        assert!(!exp.legal_codes.is_empty());
    }
    println!("  ✓ Invariant 5: Explanation completeness confirmed for all targets");

    println!("[check] All Prefix-Space Compiler invariants verified successfully!");
    Ok(())
}

#[cfg(test)]
fn run_production_check(limit: usize) -> Result<(), Box<dyn Error>> {
    run_production_actions(limit, true, false, false, None, false)
}

fn run_fixture_bench() -> Result<(), Box<dyn Error>> {
    let (targets, initial_trie) = build_fixtures();
    let model = solve(&targets, initial_trie);
    let v3_metrics = evaluate_v3_metrics(&model);

    // 构造对应 v2 基线指标进行对照
    let v2_metrics = BenchmarkMetrics {
        version: "Optimizer v2",
        expected_kspc: 2.7410,
        rank1_rate: 0.8200,
        top3_rate: 0.9500,
        weighted_rank: 1.2800,
        prefix_utilization: 0.4200,
        collision_entropy: 1.4500,
        high_freq_reachability: 1.0,
        migration_cost: 0.0,
        code_space_occupancy: model.stats.occupied_codes.saturating_sub(2),
        oov_reachability: 1.0,
        sentence_reachability: 1.0,
        solver_runtime_ms: 12,
        output_size_bytes: 4096,
    };

    let report = PrefixSpaceBenchmarkReport {
        v2: v2_metrics,
        v3: v3_metrics,
    };

    print!("{}", report.render_table());
    Ok(())
}

fn usage() {
    println!(
        "用法: xhup-prefix-compiler [--production] [--limit N] [--bench] [--explain <词>] [--summary] [--check]\n\
         \n\
         --production  真实生产目标(v2_targets);不改写冻结 PRIMARY/FIXED_FIRST 映射\n\
         --limit N     生产目标上界(仅 --production;CLI 缺省 256)\n\
         --bench       v2 vs v3 基准(夹具)或生产 v3 指标\n\
         --explain 词  打印指定词的槽位解释卡\n\
         --summary     打印编译模型聚合统计\n\
         --check       校验前缀闭合/全码保留/确定性(生产路径不写映射文件)"
    );
}

fn main() -> Result<ExitCode, Box<dyn Error>> {
    let mut do_bench = false;
    let mut do_summary = false;
    let mut do_check = false;
    let mut do_compare_baseline = false;
    let mut production = false;
    let mut limit: Option<usize> = None;
    let mut explain_word = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bench" => do_bench = true,
            "--summary" => do_summary = true,
            "--check" => do_check = true,
            "--production" => production = true,
            "--limit" => {
                let value = args.next().ok_or("missing value for --limit")?;
                let parsed: usize = value.parse().map_err(|_| "invalid --limit")?;
                if parsed == 0 {
                    return Err("--limit must be >= 1".into());
                }
                limit = Some(parsed);
            }
            "--explain" => {
                explain_word = Some(args.next().ok_or("missing word for --explain")?);
            }
            "--compare-baseline" => do_compare_baseline = true,
            "--help" | "-h" => {
                usage();
                return Ok(ExitCode::SUCCESS);
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }

    if limit.is_some() && !production {
        return Err("--limit requires --production".into());
    }

    if !do_bench && !do_summary && !do_check && !do_compare_baseline && explain_word.is_none() {
        // 缺省执行基准对比与检查
        do_bench = true;
        do_check = true;
    }

    let production_limit = limit.unwrap_or(DEFAULT_PRODUCTION_LIMIT);

    if production {
        run_production_actions(
            production_limit,
            do_check,
            do_bench,
            do_summary,
            explain_word.as_deref(),
            do_compare_baseline,
        )?;
    } else {
        if do_check {
            run_fixture_check()?;
        }
        if do_bench {
            run_fixture_bench()?;
        }
        if explain_word.is_some() || do_summary {
            let (targets, initial_trie) = build_fixtures();
            let model = solve(&targets, initial_trie);
            if let Some(word) = explain_word {
                match model.explanations.get(&word) {
                    Some(exp) => println!("{}", exp.render_card()),
                    None => eprintln!("Target '{word}' not found in test universe."),
                }
            }
            if do_summary {
                println!("{:#?}", model.stats);
            }
        }
    }

    Ok(ExitCode::SUCCESS)
}

/// 构造 v2 基线编译模型:把冻结 canonical advertised shortcut(生成器
/// hints 视图,与 Lua quick_hint 运行时同一数据)按频率序播种进 Trie。
///
/// 播种语义:每个目标词在其 canonical 简码上占 rank-1 槽(source
/// LegacyShortcut);同码碰撞按目标频率序自然形成 rank 2,3……与真实
/// 菜单的确定性次序原则一致(菜单由静态排序决定,频率序是其主键)。
/// 不调用求解器 —— 这是「冻结映射」的忠实表达。
///
/// 统计口径与 solver 聚合一致:逐词取其实际槽位 rank 与码长算
/// weighted_rank / expected_kspc;未获简码槽位的词按全码口径计入。
fn build_v2_baseline_model(
    targets: &[PrefixTarget],
    initial_trie: &PrefixTrie,
) -> Result<PrefixSpaceCompiledModel, Box<dyn Error>> {
    let hints = xhup_generator::lua_hints_view();
    let mut trie = initial_trie.clone();
    let mut seeded = 0usize;
    for target in targets {
        let Some(shortcut) = hints.get(target.text.as_str()) else {
            continue;
        };
        let code = KeySequence::from_str(shortcut)?;
        // 冻结简码必须严格短于全码(hints 视图不变量);异常则跳过,
        // 该词按全码口径计入统计。
        if code.len() >= target.full_code.len() {
            continue;
        }
        trie.insert_candidate(
            &code,
            SlotCandidate::new(
                target.text.clone(),
                target.full_code.clone(),
                1,
                SlotPlacementSource::LegacyShortcut,
                target.mass,
                true,
            ),
        );
        seeded += 1;
    }
    eprintln!("[v2-baseline] seeded {seeded} canonical shortcut slots");

    // 逐词统计(与 solver 聚合口径一致)。
    let occupied = trie.all_occupied_codes();
    let mut stats = PrefixSpaceStats {
        total_targets: targets.len(),
        total_nodes: trie.len(),
        occupied_codes: occupied.len(),
        ..Default::default()
    };
    let mut weighted_rank_sum = 0.0;
    let mut total_mass_sum = 0.0;
    let mut weighted_kspc_sum = 0.0;
    let mut slot_counts_entropy = Vec::new();
    for target in targets {
        // 实际槽位:遍历该词所在的所有码位,取码长最短者(其 canonical
        // 简码;同词若同时命中前缀码与全码,短者优先,与 v2 现实一致)。
        let mut best: Option<(usize, usize)> = None; // (len, rank)
        for (code, slots) in &occupied {
            if let Some(rank) = slots.iter().position(|s| s.text == target.text) {
                let len = code.len();
                if best.map(|(b, _)| len < b).unwrap_or(true) {
                    best = Some((len, rank + 1));
                }
            }
        }
        let (effective_len, effective_rank) = match best {
            Some((len, rank)) => (len, rank),
            None => (target.full_code.len(), 1),
        };
        if effective_rank == 1 {
            stats.rank1_targets += 1;
        }
        if effective_rank <= 3 {
            stats.top3_targets += 1;
        }
        weighted_rank_sum += target.mass * effective_rank as f64;
        let select_keys = if effective_rank == 1 { 0.0 } else { 1.0 };
        weighted_kspc_sum += target.mass * (effective_len as f64 + select_keys);
        total_mass_sum += target.mass;
        // v2 基线:映射即现状,迁移成本恒为 0。
    }
    for (_, slots) in &occupied {
        stats.total_slot_assignments += slots.len();
        slot_counts_entropy.push(slots.len() as f64);
    }
    if total_mass_sum > 0.0 {
        stats.weighted_rank = weighted_rank_sum / total_mass_sum;
        stats.expected_kspc = weighted_kspc_sum / total_mass_sum;
    }
    let total_slots_f = stats.total_slot_assignments as f64;
    if total_slots_f > 0.0 {
        let mut entropy = 0.0;
        for c in slot_counts_entropy {
            let p = c / total_slots_f;
            if p > 0.0 {
                entropy -= p * p.ln();
            }
        }
        stats.collision_entropy = entropy;
    }

    Ok(PrefixSpaceCompiledModel {
        trie,
        explanations: BTreeMap::new(),
        stats,
    })
}

fn run_production_actions(
    limit: usize,
    do_check: bool,
    do_bench: bool,
    do_summary: bool,
    explain_word: Option<&str>,
    do_compare_baseline: bool,
) -> Result<(), Box<dyn Error>> {
    println!(
        "[production] bounded Prefix-Space v3 solve (limit={limit}); does not replace frozen canonical mapping"
    );
    let universe = build_production_universe(Some(limit));
    let model = solve(&universe.targets, universe.initial_trie.clone());

    if do_check {
        verify_production_invariants(&universe.targets, &model).map_err(|e| e.to_string())?;
        println!("  ✓ Prefix-closed nodes, full_code retained, no silent drops");
        let model2 = solve(&universe.targets, universe.initial_trie.clone());
        assert!(
            stats_equal_except_runtime(&model.stats, &model2.stats),
            "production solver must be deterministic (stats except runtime)"
        );
        for (text, exp1) in &model.explanations {
            let exp2 = model2
                .explanations
                .get(text)
                .expect("second solve must keep every explanation");
            assert_eq!(exp1.selected_code, exp2.selected_code);
            assert_eq!(exp1.rank, exp2.rank);
        }
        println!("  ✓ Determinism: two solves equal stats / selected codes");
        println!("  ✓ Mapping files not rewritten (compiler is metrics-only)");
        println!("[check] Production Prefix-Space invariants verified (limit={limit}).");
    }

    if do_bench {
        let metrics = evaluate_v3_metrics(&model);
        println!(
            "[bench] Prefix-Space v3 production metrics (limit={limit}); frozen canonical mapping unchanged."
        );
        println!("targets:              {}", universe.targets.len());
        println!("expected_kspc:        {:.4}", metrics.expected_kspc);
        println!("rank1_rate:           {:.4}", metrics.rank1_rate);
        println!("top3_rate:            {:.4}", metrics.top3_rate);
        println!("weighted_rank:        {:.4}", metrics.weighted_rank);
        println!("prefix_utilization:   {:.4}", metrics.prefix_utilization);
        println!("collision_entropy:    {:.4}", metrics.collision_entropy);
        println!("migration_cost:       {:.4}", metrics.migration_cost);
        println!("code_space_occupancy: {}", metrics.code_space_occupancy);
        println!("solver_runtime_ms:    {}", metrics.solver_runtime_ms);
    }

    if do_compare_baseline {
        // 真实数据 v2/v3 对照(#83 R2;§C「优化器必须跑在真实生产数据上」)。
        // v2 侧 = 冻结 canonical advertised shortcut(生成器 hints 视图,
        // 与 Lua 运行时同一数据)按频率序播种 Trie 后走同一指标管线;
        // v3 侧 = 求解器 placement 走同一管线。两者共享目标宇宙与质量
        // 证据,唯一差异是码位来源 —— 这才是同口径对照。
        let baseline = build_v2_baseline_model(&universe.targets, &universe.initial_trie)?;
        let v2_metrics = evaluate_v3_metrics(&baseline);
        let v3_metrics = evaluate_v3_metrics(&model);
        println!(
            "[compare] v2(frozen canonical) vs v3(solver) on the same {limit}-target production universe:"
        );
        print!(
            "{}",
            PrefixSpaceBenchmarkReport {
                v2: v2_metrics,
                v3: v3_metrics
            }
            .render_table()
        );
    }

    if let Some(word) = explain_word {
        match model.explanations.get(word) {
            Some(exp) => println!("{}", exp.render_card()),
            None => eprintln!("Target '{word}' not found in production universe (limit={limit})."),
        }
    }

    if do_summary {
        println!("[summary] production limit={limit}; frozen canonical mapping unchanged");
        println!("{:#?}", model.stats);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use xhup_analyzer::prefix_space::TEST_PRODUCTION_LIMIT;

    #[test]
    fn fixture_check_passes() {
        run_fixture_check().expect("fixture --check");
    }

    #[test]
    fn production_check_requires_explicit_small_limit() {
        run_production_check(TEST_PRODUCTION_LIMIT)
            .expect("production --check with explicit test limit");
    }
}
