//! 万象字频提取器(数据 provenance/复现工具)。
//!
//! 从**本地**万象 `zi.dict.yaml` 提取落在本项目规范数据内的 `(汉字, 规范读音)`
//! 子集,把带调拼音归一化为项目规范无调读音,按 `(汉字, 规范读音)` 聚合全部
//! 声调变体的源分数,输出规范频率 TSV 到 stdout;覆盖率审计报告写到 stderr。
//!
//! 用法:
//!
//! ```console
//! cargo run -p xhup-generator --example extract_wanxiang_frequency -- \
//!     /path/to/zi.dict.yaml > data/frequency/wanxiang_reading_scores.tsv
//! ```
//!
//! 本工具不访问网络,也不在运行时感知来源 URL;来源身份(pin 的仓库、提交、
//! 路径、blob SHA、许可)作为固定元数据写入输出 TSV 的注释头。构建、测试与
//! 正常生成都以入库的 TSV 为输入,不依赖本工具。

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::process::ExitCode;

use xhup_core::XhupHanzi;

#[path = "common/wanxiang.rs"]
mod wanxiang;

use wanxiang::normalize_reading;

/// 输出 TSV 的注释头(行数行在写出前追加)。
const HEADER: &str = "\
# XHUP Flow 规范读音频率数据:万象 / RIME-LMDG 单字源分数的规范子集
# source_repo: amzxyz/rime-wanxiang
# source_commit: 7ec998b28c9a5c57260d2ba24b264c1c1820e0ef
# source_path: dicts/zi.dict.yaml
# source_blob_sha: 9a69cb891f2e0c158313d14e0ea6c3925ca081ef
# source_license: CC-BY-4.0
# semantic_source: amzxyz/RIME-LMDG
# normalization: 去除声调(āáǎà→a 等五组);ü 族(üǖǘǚǜ)→v;ńňǹ→n;ḿ→m;
#   归一化后仍含非 a-z 字符的源读音视为坏行忽略(如分解形式的 m+U+0300)
# aggregation: 归一化后落到同一 (汉字, 规范读音) 的全部源行分数按 u64 校验和聚合
# match_policy: 仅保留汉字属于规范 8105 清单且归一化读音等于该字某个规范读音的源行;
#   不发明新读音,规范汉字/读音数据仍是成员资格的唯一事实来源
";

fn main() -> ExitCode {
    let mut args = env::args_os();
    let program = args.next().unwrap_or_default();
    let Some(path) = args.next() else {
        eprintln!(
            "用法: {} <本地 zi.dict.yaml 路径> > wanxiang_reading_scores.tsv",
            program.to_string_lossy()
        );
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("只接受一个参数:本地 zi.dict.yaml 路径");
        return ExitCode::from(2);
    }

    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("无法读取 {}: {err}", path.to_string_lossy());
            return ExitCode::FAILURE;
        }
    };

    let report = extract(&text);

    // 覆盖率审计:规范读音关系总数、匹配数、缺失清单(stderr)。
    let mut canonical_total = 0usize;
    let mut missing: Vec<(char, &str)> = Vec::new();
    for &hanzi in XhupHanzi::all() {
        for &reading in hanzi.readings() {
            canonical_total += 1;
            if !report
                .scores
                .contains_key(&(hanzi.as_char(), reading.as_str()))
            {
                missing.push((hanzi.as_char(), reading.as_str()));
            }
        }
    }
    let matched = report.scores.len();
    let percent = matched as f64 * 100.0 / canonical_total as f64;
    eprintln!("源数据行: {} 个三字段行", report.source_rows);
    eprintln!("坏行忽略(归一化后仍含非 a-z): {}", report.malformed);
    eprintln!("丢弃(汉字不在规范清单): {} 行", report.noncanonical_hanzi);
    eprintln!(
        "丢弃(读音不是该字规范读音): {} 行",
        report.noncanonical_reading
    );
    eprintln!("规范读音关系: {canonical_total}");
    eprintln!("匹配关系(输出 TSV 行数): {matched}");
    eprintln!("缺失关系: {}(覆盖率 {percent:.2}%)", missing.len());
    for (zi, reading) in &missing {
        eprintln!("  缺失: {zi}\t{reading}");
    }

    print!("{HEADER}");
    println!("# rows: {matched}");
    for ((zi, reading), score) in &report.scores {
        println!("{zi}\t{reading}\t{score}");
    }
    ExitCode::SUCCESS
}

/// 提取结果:聚合后的分数表与审计计数。
struct ExtractReport {
    /// `(汉字, 规范读音) -> 聚合分数`,BTreeMap 保证确定性升序。
    scores: BTreeMap<(char, &'static str), u64>,
    source_rows: usize,
    malformed: usize,
    noncanonical_hanzi: usize,
    noncanonical_reading: usize,
}

/// 解析万象源文本并聚合;源行格式 `汉字<TAB>带调拼音<TAB>分数`。
fn extract(text: &str) -> ExtractReport {
    let mut scores: BTreeMap<(char, &'static str), u64> = BTreeMap::new();
    let mut report = ExtractReport {
        scores: BTreeMap::new(),
        source_rows: 0,
        malformed: 0,
        noncanonical_hanzi: 0,
        noncanonical_reading: 0,
    };

    for line in text.lines() {
        let mut fields = line.split('\t');
        let (Some(char_field), Some(pinyin), Some(score_field), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue; // YAML 头/注释/分隔线:非三字段行
        };
        report.source_rows += 1;

        let mut chars = char_field.chars();
        let (Some(zi), None) = (chars.next(), chars.next()) else {
            report.malformed += 1;
            continue;
        };
        let Ok(score) = score_field.parse::<u64>() else {
            report.malformed += 1;
            continue;
        };
        let Some(normalized) = normalize_reading(pinyin) else {
            report.malformed += 1;
            continue;
        };

        let Ok(hanzi) = XhupHanzi::try_from(zi) else {
            report.noncanonical_hanzi += 1;
            continue;
        };
        // 归一化读音必须等于该字的某个规范读音;规范数据不因此发明新读音。
        let Some(&reading) = hanzi
            .readings()
            .iter()
            .find(|reading| reading.as_str() == normalized)
        else {
            report.noncanonical_reading += 1;
            continue;
        };

        let entry = scores
            .entry((hanzi.as_char(), reading.as_str()))
            .or_insert(0);
        *entry = entry
            .checked_add(score)
            .expect("聚合分数 u64 溢出:源数据超出预期规模");
    }
    report.scores = scores;
    report
}
