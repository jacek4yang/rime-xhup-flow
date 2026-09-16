//! `production-shortcut-audit`:全量 advertised shortcut 审计 CLI
//! (Issue #83 §3/§9/§10;§24 DoD 的 misleading-hint rate 测量程序)。
//!
//! 用法:`production-shortcut-audit [--output <TSV>] [--threshold <rate>]`
//!
//! - 审计对象:生成器 quick-hint 提示视图(与 Lua quick_hint 运行时同一
//!   实现,§9 效用语义统一),绝不手工指定词(§21);
//! - rank 来源:canonical 真实菜单占用(CodeOccupancy,与
//!   static-shortcut-audit --dump-static-menu-manifest 同源);
//! - 输出:确定性 TSV(头部聚合指标 + 逐条判定);
//! - `--threshold`:misleading-hint rate 门禁(0 ≤ t ≤ 1);超标即退出码 1。
//!   CI 以 `--threshold 0.0` 断言「misleading = 0」(§3 合同)。

use std::process::ExitCode;

use xhup_analyzer::shortcut_audit::{ShortcutAuditInput, audit_tsv, run_audit};

fn usage() -> ! {
    eprintln!(
        "用法: production-shortcut-audit [--output <TSV>] [--threshold <rate>]\n\
         \n\
         审计全部 advertised shortcut(quick-hint 提示视图)对真实菜单的\n\
         rank/节省/判定,产出确定性 TSV 与聚合指标(头部注释)。"
    );
    std::process::exit(2);
}

fn main() -> ExitCode {
    let mut output: Option<String> = None;
    let mut threshold: Option<f64> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => output = Some(args.next().unwrap_or_else(|| usage())),
            "--threshold" => {
                let raw = args.next().unwrap_or_else(|| usage());
                threshold = Some(raw.parse().unwrap_or_else(|_| usage()));
            }
            _ => usage(),
        }
    }
    if let Some(t) = threshold
        && !(0.0..=1.0).contains(&t)
    {
        eprintln!("threshold 必须在 0..=1: {t}");
        return ExitCode::FAILURE;
    }

    let occupancy = xhup_analyzer::occupancy::CodeOccupancy::build_current_production();
    let (hints, full_lens) = xhup_generator::lua_hints_view_with_full_lens();
    // 词频:万象归一化(与 evidence/sweep 同口径)。
    let data = xhup_analyzer::build_analysis();
    let total: f64 = data.words.iter().map(|e| e.frequency_score() as f64).sum();
    let normalized: std::collections::BTreeMap<String, f64> = data
        .words
        .iter()
        .map(|e| (e.word().to_string(), e.frequency_score() as f64 / total))
        .collect();

    let input = ShortcutAuditInput {
        hints: &hints,
        full_code_lens: &full_lens,
        normalized_frequency: &normalized,
        occupancy: &occupancy,
        top_n: 1000,
    };
    let (entries, metrics) = run_audit(&input);
    let tsv = audit_tsv(&entries, &metrics);

    let misleading_rate = metrics.misleading as f64 / metrics.total.max(1) as f64;
    match &output {
        Some(path) => {
            if let Err(e) = std::fs::write(path, &tsv) {
                eprintln!("无法写出 {}: {e}", path);
                return ExitCode::FAILURE;
            }
            eprintln!("[shortcut-audit] TSV → {path}");
        }
        None => print!("{tsv}"),
    }
    eprintln!(
        "[shortcut-audit] total={} useful={} useful_with_selection={} misleading={} \
         misleading_rate={:.6} expected_effort_saving={:.3} collision_mass={:.3} \
         prefix_utilization={:.3} top1000_shallow_coverage={:.3}",
        metrics.total,
        metrics.useful,
        metrics.useful_with_selection,
        metrics.misleading,
        misleading_rate,
        metrics.expected_effort_saving,
        metrics.collision_mass,
        metrics.prefix_utilization,
        metrics.top1000_shallow_coverage,
    );
    if let Some(t) = threshold {
        if misleading_rate > t {
            eprintln!("[shortcut-audit] FAIL: misleading rate {misleading_rate:.6} > 门禁 {t}");
            return ExitCode::FAILURE;
        }
        eprintln!("[shortcut-audit] PASS: misleading rate ≤ {t}");
    }
    ExitCode::SUCCESS
}
