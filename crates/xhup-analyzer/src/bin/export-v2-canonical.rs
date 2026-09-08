//! `export-v2-canonical`:v2 映射 dump → canonical 候选文件确定性导出。
//!
//! 用法:`export-v2-canonical --dump <sweep-v2 dump TSV> --out-dir <目录>
//! [--point <运行点标签>] [--date <YYYY-MM-DD>] [--dry-run]`
//!
//! 产物:`word_fixed_first.tsv`(占用码 rank1 且 FF 格式合规)与
//! `word_shortcuts_primary.tsv`(其余全部),含 provenance 头注释
//! (运行点/参数/日期/输入 dump SHA256)。输出确定性(同输入同
//! provenance → 字节一致);`--dry-run` 只打印统计不写文件。
//! dump 是本地文件(绝不入库),canonical 词层与占用视图来自库内数据。

use std::path::PathBuf;
use std::process::ExitCode;

use xhup_analyzer::export_v2::{self, Provenance};
use xhup_analyzer::occupancy::CodeOccupancy;

fn usage() -> ! {
    eprintln!(
        "用法: export-v2-canonical --dump <v2 dump TSV> --out-dir <目录>\n\
         \x20             [--point <运行点标签>] [--date <YYYY-MM-DD>] [--dry-run]\n\
         \n\
         --dump     sweep-v2 --dump-mapping 产物(词/码/rank/效用分解)。\n\
         --out-dir  输出目录(写 word_fixed_first.tsv 与 word_shortcuts_primary.tsv)。\n\
         --point    运行点标签;缺省从 dump 文件名还原(`_` → `|`)。\n\
         --date     生成日期(写入 provenance);缺省为今天。\n\
         --dry-run  只打印统计,不写文件。"
    );
    std::process::exit(2);
}

/// 系统时间 → YYYY-MM-DD(civil-from-days,无依赖)。
fn today() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("系统时间在纪元之后")
        .as_secs()
        / 86_400;
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// 从运行点标签派生参数摘要行(标签即参数编码,见 sweep_v2::grid)。
fn parameters_of(label: &str) -> String {
    format!("rank_curve/ambiguity/disruption/xhup_deviation/evidence = {label}")
}

fn main() -> ExitCode {
    let mut dump_path: Option<PathBuf> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut point: Option<String> = None;
    let mut date: Option<String> = None;
    let mut dry_run = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dump" => dump_path = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            "--out-dir" => out_dir = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            "--point" => point = Some(args.next().unwrap_or_else(|| usage())),
            "--date" => date = Some(args.next().unwrap_or_else(|| usage())),
            "--dry-run" => dry_run = true,
            _ => usage(),
        }
    }
    let (Some(dump_path), Some(out_dir)) = (dump_path, out_dir) else {
        usage();
    };

    let dump_text =
        std::fs::read_to_string(&dump_path).unwrap_or_else(|e| panic!("dump 不可读: {e}"));
    let entries =
        export_v2::parse_dump(&dump_text).unwrap_or_else(|e| panic!("dump 解析失败: {e}"));
    let point_label = point.unwrap_or_else(|| {
        dump_path
            .file_stem()
            .expect("dump 应有文件名")
            .to_string_lossy()
            .replace('_', "|")
    });
    let provenance = Provenance {
        parameters: parameters_of(&point_label),
        point_label,
        date: date.unwrap_or_else(today),
        dump_sha256: export_v2::sha256_hex(dump_text.as_bytes()),
    };

    // canonical 词层(词 → 全码)与 baseline 固定层占用码集合。
    let word_full_codes: std::collections::BTreeMap<_, _> =
        xhup_generator::word_code_analysis_entries()
            .iter()
            .map(|e| (e.word().to_string(), e.code().clone()))
            .collect();
    let occupancy = CodeOccupancy::build_baseline_fixed();
    let baseline_codes: std::collections::BTreeSet<_> =
        occupancy.occupied_codes().cloned().collect();

    let output = match export_v2::export(&entries, &word_full_codes, &baseline_codes, &provenance) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("导出校验失败: {error}");
            return ExitCode::FAILURE;
        }
    };

    let stats = &output.stats;
    println!("运行点: {}", provenance.point_label);
    println!("dump sha256: {}", provenance.dump_sha256);
    println!(
        "FIXED_FIRST: {} 条(占用码 rank1,FF 格式合规)",
        stats.fixed_first
    );
    println!(
        "PRIMARY: {} 条(其中占用码上: {} 条)",
        stats.primary, stats.primary_occupied_tail
    );
    println!(
        "码长分布: {}",
        stats
            .by_length
            .iter()
            .map(|(l, n)| format!("{l}键:{n}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    println!(
        "rank 分布: {}",
        stats
            .by_rank
            .iter()
            .map(|(r, n)| format!("r{r}:{n}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    if dry_run {
        println!("(dry-run,未写文件)");
        return ExitCode::SUCCESS;
    }
    std::fs::create_dir_all(&out_dir).expect("输出目录可建");
    std::fs::write(
        out_dir.join("word_fixed_first.tsv"),
        &output.fixed_first_tsv,
    )
    .expect("FF TSV 可写");
    std::fs::write(
        out_dir.join("word_shortcuts_primary.tsv"),
        &output.primary_tsv,
    )
    .expect("primary TSV 可写");
    println!("已写入 {}", out_dir.display());
    ExitCode::SUCCESS
}
