//! `shortcut-oracle`:optimality-gap 研究 CLI(Issue #83 §8,§25 第 5 步)。
//!
//! 用法:`shortcut-oracle [--limit <N>] [--output <JSON>]`
//!
//! 对 canonical v2 全部 targets 构建单码 oracle 实例(生产同款评分),
//! 运行贪心 vs 精确匈牙利匹配,输出 optimality-gap 报告。研究用途,
//! 不参与生产构建;输出确定性。

use std::process::ExitCode;

fn usage() -> ! {
    eprintln!(
        "用法: shortcut-oracle [--limit <N>] [--output <JSON>]\n\
         \n\
         对 canonical targets(默认全部;--limit 取高频前 N)构建 oracle\n\
         实例,比较效用降序贪心与精确匈牙利匹配,输出 optimality gap。"
    );
    std::process::exit(2);
}

fn main() -> ExitCode {
    let mut limit: Option<usize> = None;
    let mut output: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--limit" => {
                limit = Some(
                    args.next()
                        .unwrap_or_else(|| usage())
                        .parse()
                        .unwrap_or_else(|_| usage()),
                );
            }
            "--output" => output = Some(args.next().unwrap_or_else(|| usage())),
            _ => usage(),
        }
    }

    let data = xhup_analyzer::build_analysis();
    let evidence = {
        let set = xhup_analyzer::evidence::LexicalEvidenceSet::build(&data.words, &data.frequency);
        xhup_analyzer::sweep_v2::evidence_by_word(&set)
    };
    let scale = xhup_analyzer::mapping_v2::MassScale::build(&evidence);
    let baseline = xhup_analyzer::mapping_v2::BaselineMassView::build(
        &xhup_analyzer::occupancy::CodeOccupancy::build_current_production(),
    );
    let cost = xhup_analyzer::optimizer_v2::CostModelV2::default();
    let weights = xhup_analyzer::optimizer_v2::EvidenceWeights::default();
    let mut targets = xhup_analyzer::sweep_v2::v2_targets(&data.words);
    targets.sort_by_key(|t| std::cmp::Reverse(t.frequency_score()));
    if let Some(n) = limit {
        targets.truncate(n);
    }

    // 生产同款评分(阶段 1 语义;占用质量以 baseline 为快照)。
    let mut scored = Vec::new();
    for target in &targets {
        let Some(ev) = evidence.get(target.word()) else {
            continue;
        };
        let mass = scale.mass(ev, &weights);
        let baseline_breakdown = xhup_analyzer::optimizer_v2::evaluate_assignment(
            ev,
            &xhup_analyzer::optimizer_v2::CandidateSlot {
                key_len: target.full_code().len(),
                rank: baseline.full_code_rank(target.word()),
                occupant_mass: 0.0,
                displaced_mass: 0.0,
            },
            &cost,
            &weights,
            xhup_analyzer::xhup_prior::NEUTRAL_PRIOR,
            target.full_code().len(),
            true,
        );
        let baseline_total = baseline_breakdown.total() - baseline_breakdown.xhup_prior;
        for candidate in target.candidates() {
            let code = candidate.shortcut_code();
            let pattern_consistent = target.full_code().as_slice().starts_with(code.as_slice());
            // 无 v2 members 的静态快照(baseline 组内排序;与阶段 1 一致)。
            let mut before = 0usize;
            let mut displaced = 0.0;
            let mut total = 0.0;
            for m in baseline.group(code) {
                total += m;
                if m.total_cmp(&mass).is_gt() {
                    before += 1;
                } else {
                    displaced += *m;
                }
            }
            let breakdown = xhup_analyzer::optimizer_v2::evaluate_assignment(
                ev,
                &xhup_analyzer::optimizer_v2::CandidateSlot {
                    key_len: code.len(),
                    rank: before + 1,
                    occupant_mass: total,
                    displaced_mass: displaced,
                },
                &cost,
                &weights,
                xhup_analyzer::xhup_prior::NEUTRAL_PRIOR,
                target.full_code().len(),
                pattern_consistent,
            );
            scored.push((
                target.word().to_string(),
                code.clone(),
                before + 1,
                breakdown.total() - baseline_total,
            ));
        }
    }

    let instance = xhup_analyzer::shortcut_oracle::OracleInstance::from_scored(scored);
    let comparison = xhup_analyzer::shortcut_oracle::compare_greedy_vs_exact(&instance);

    let report = serde_json::json!({
        "schema": "shortcut-oracle-gap/v1",
        "words": comparison.instance.words,
        "slots": comparison.instance.slots,
        "greedy_utility": comparison.greedy_utility,
        "exact_utility": comparison.exact_utility,
        "optimality_gap": comparison.optimality_gap,
        "relative_gap": comparison.relative_gap,
        "words_lost_by_greedy": comparison.words_lost_by_greedy,
    });
    let text = serde_json::to_string_pretty(&report).expect("report 可序列化") + "\n";
    match &output {
        Some(path) => {
            std::fs::write(path, &text).expect("报告可写");
            eprintln!("[shortcut-oracle] 报告 → {path}");
        }
        None => print!("{text}"),
    }
    eprintln!(
        "[shortcut-oracle] words={} slots={} greedy={:.4} exact={:.4} gap={:.6} ({:.4}%) lost={}",
        comparison.instance.words,
        comparison.instance.slots,
        comparison.greedy_utility,
        comparison.exact_utility,
        comparison.optimality_gap,
        comparison.relative_gap * 100.0,
        comparison.words_lost_by_greedy,
    );
    ExitCode::SUCCESS
}
