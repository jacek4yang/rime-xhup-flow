//! 万象基础词库提取器(数据 provenance/复现工具)。
//!
//! 从**本地**万象 `jichu.dict.yaml` 提取落在本项目规范数据内的 2~4 字高频词语
//! 子集,把带调拼音序列归一化为项目规范无调读音序列,按 `(词, 规范读音序列)`
//! 去重并聚合源分数,再按 collision policy 与 top-N 选择输出规范词语 TSV 到
//! stdout;审计报告写到 stderr。
//!
//! 用法:
//!
//! ```console
//! cargo run -p xhup-generator --example extract_wanxiang_words -- \
//!     /path/to/jichu.dict.yaml > data/words/wanxiang_base_words.tsv
//! ```
//!
//! 本工具不访问网络,也不在运行时感知来源 URL;来源身份(pin 的仓库、提交、
//! 路径、blob SHA、许可)作为固定元数据写入输出 TSV 的注释头。构建、测试与
//! 正常生成都以入库的 TSV 为输入,不依赖本工具。
//!
//! collision policy:二字词的 4 键码按 **semantic entry**(词 + 读音序列)粒度
//! 与规范单字全码集比对——只有推导码冲突的那一条 semantic entry 被排除;同一
//! 词形若存在不冲突的合法读音序列,仍然保留。

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::process::ExitCode;

use xhup_core::{HanziReading, XhupHanzi};
use xhup_generator::canonical_char_entries;

#[path = "common/wanxiang.rs"]
mod wanxiang;

use wanxiang::normalize_reading;

/// 各词长的 top-N 选择目标(词长 → 数量)。
const TARGETS: [(usize, usize); 3] = [(2, 50_000), (3, 30_000), (4, 20_000)];

/// 输出 TSV 的注释头(行数行在写出前追加)。
const HEADER: &str = "\
# XHUP Flow 规范高频词语数据:万象 / RIME-LMDG 基础词库的规范子集
# source_repo: amzxyz/rime-wanxiang
# source_commit: 4618d67a978ff4f41b165c10b35558d38e333ab1
# source_path: dicts/jichu.dict.yaml
# source_blob_sha: a0f66e2fc6130f3f1c9b2e5109644c8b893477b0
# source_license: CC-BY-4.0
# semantic_source: amzxyz/RIME-LMDG
# normalization: 与字频数据一致(去声调五组;ü 族→v;ńňǹ→n;ḿ→m;
#   归一化后仍含非 a-z 字符的源读音视为坏行忽略)
# aggregation: 归一化后落到同一 (词, 规范读音序列) 的全部源行分数按 u64 校验和聚合
# match_policy: 每字属于规范 8105 清单、对应读音是该字规范读音且可编码为
#   XHUP 输入音节;不发明新读音
# collision_policy: 二字词按 semantic entry(词 + 读音序列)推导 4 键码,
#   与规范单字全码集冲突的 semantic entry 被排除;同词形的不冲突读音序列保留
# selection: 各词长独立按 (分数降序, 词 Unicode 升序, 读音序列升序) 取
#   前 50000 / 30000 / 20000 条;合法候选不足目标即失败,不静默缩水
# serialization: 词长升序 → 词 Unicode 升序 → 读音序列升序
";

/// 一条通过规范校验的 semantic entry:词形 + 逐字规范读音 + 聚合分数。
#[derive(Debug)]
struct SemanticEntry {
    word: String,
    readings: Vec<HanziReading>,
    score: u64,
}

impl SemanticEntry {
    /// 推导精确 XHUP 词码:逐字规范读音 → 双拼两键 → 按字序拼接。
    fn code(&self) -> String {
        let mut out = String::with_capacity(self.readings.len() * 2);
        for &reading in &self.readings {
            let syllable = reading
                .to_input_syllable()
                .expect("semantic entry 不变量:读音必然可编码");
            out.push_str(&syllable.to_double_pinyin_code().to_string());
        }
        out
    }

    /// 词长(Unicode 标量数),等于读音数。
    fn word_len(&self) -> usize {
        self.readings.len()
    }

    /// 选择排序:(分数降序, 词 Unicode 升序, 读音序列升序)。
    fn selection_cmp(a: &Self, b: &Self) -> std::cmp::Ordering {
        b.score
            .cmp(&a.score)
            .then_with(|| a.word.cmp(&b.word))
            .then_with(|| a.readings.cmp(&b.readings))
    }
}

/// 提取结果:聚合后的 semantic entry 表与审计计数。
struct ExtractReport {
    /// `(词, 规范读音序列) -> 聚合分数`,BTreeMap 保证确定性。
    scores: BTreeMap<(String, Vec<HanziReading>), u64>,
    source_rows: usize,
    malformed: usize,
    length_filtered: usize,
    reading_count_mismatch: usize,
    noncanonical_hanzi: usize,
    noncanonical_reading: usize,
    unencodable: usize,
}

/// 逐字规范校验的失败类别(用于审计计数)。
enum Reject {
    Malformed,
    NoncanonicalHanzi,
    NoncanonicalReading,
    Unencodable,
}

/// 解析万象源文本并聚合;源行格式 `词<TAB>带调拼音序列<TAB>分数`。
fn extract(text: &str) -> ExtractReport {
    let mut scores: BTreeMap<(String, Vec<HanziReading>), u64> = BTreeMap::new();
    let mut report = ExtractReport {
        scores: BTreeMap::new(),
        source_rows: 0,
        malformed: 0,
        length_filtered: 0,
        reading_count_mismatch: 0,
        noncanonical_hanzi: 0,
        noncanonical_reading: 0,
        unencodable: 0,
    };

    for line in text.lines() {
        let mut fields = line.split('\t');
        let (Some(word_field), Some(pinyin_field), Some(score_field), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue; // YAML 头/注释/分隔线:非三字段行
        };
        report.source_rows += 1;

        let chars: Vec<char> = word_field.chars().collect();
        if !(2..=4).contains(&chars.len()) {
            report.length_filtered += 1;
            continue;
        }
        let score = match score_field.parse::<u64>() {
            Ok(score) if score > 0 => score,
            _ => {
                report.malformed += 1;
                continue;
            }
        };

        let pinyins: Vec<&str> = pinyin_field.split(' ').collect();
        if pinyins.len() != chars.len() {
            report.reading_count_mismatch += 1;
            continue;
        }

        // 逐字规范校验:任一环节失败则整行丢弃(读音数已对齐,逐字配对)。
        let mut readings: Vec<HanziReading> = Vec::with_capacity(chars.len());
        let mut reject = None;
        for (&zi, &pinyin) in chars.iter().zip(&pinyins) {
            let step = (|| {
                let normalized = normalize_reading(pinyin).ok_or(Reject::Malformed)?;
                let hanzi = XhupHanzi::try_from(zi).map_err(|_| Reject::NoncanonicalHanzi)?;
                let reading = hanzi
                    .readings()
                    .iter()
                    .copied()
                    .find(|reading| reading.as_str() == normalized)
                    .ok_or(Reject::NoncanonicalReading)?;
                if reading.to_input_syllable().is_none() {
                    return Err(Reject::Unencodable);
                }
                readings.push(reading);
                Ok(())
            })();
            if let Err(kind) = step {
                reject = Some(kind);
                break;
            }
        }
        match reject {
            None => {}
            Some(Reject::Malformed) => report.malformed += 1,
            Some(Reject::NoncanonicalHanzi) => report.noncanonical_hanzi += 1,
            Some(Reject::NoncanonicalReading) => report.noncanonical_reading += 1,
            Some(Reject::Unencodable) => report.unencodable += 1,
        }
        if reject.is_some() {
            continue;
        }

        let entry = scores
            .entry((word_field.to_string(), readings))
            .or_insert(0);
        *entry = entry
            .checked_add(score)
            .expect("聚合分数 u64 溢出:源数据超出预期规模");
    }
    report.scores = scores;
    report
}

fn main() -> ExitCode {
    let mut args = env::args_os();
    let program = args.next().unwrap_or_default();
    let Some(path) = args.next() else {
        eprintln!(
            "用法: {} <本地 jichu.dict.yaml 路径> > wanxiang_base_words.tsv",
            program.to_string_lossy()
        );
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("只接受一个参数:本地 jichu.dict.yaml 路径");
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
    eprintln!("源数据行: {} 个三字段行", report.source_rows);
    eprintln!("丢弃(词长不在 2..=4): {} 行", report.length_filtered);
    eprintln!(
        "坏行忽略(分数非法 / 归一化后仍含非 a-z): {}",
        report.malformed
    );
    eprintln!(
        "丢弃(拼音数与字数不一致): {} 行",
        report.reading_count_mismatch
    );
    eprintln!("丢弃(汉字不在规范清单): {} 行", report.noncanonical_hanzi);
    eprintln!(
        "丢弃(读音不是该字规范读音): {} 行",
        report.noncanonical_reading
    );
    eprintln!(
        "丢弃(读音不可编码为 XHUP 输入音节): {} 行",
        report.unencodable
    );

    // 规范单字全码集:二字词 collision 比对的基准(复用公共推导,无第二份实现)。
    let fullcodes: BTreeSet<String> = canonical_char_entries()
        .iter()
        .map(|entry| entry.code().to_string())
        .collect();
    eprintln!("规范单字全码 distinct 数: {}", fullcodes.len());

    // collision 过滤(semantic entry 粒度)并按词长分池。
    let mut pools: [Vec<SemanticEntry>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut two_char_before = 0usize;
    let mut excluded = 0usize;
    let mut collided_codes: BTreeSet<String> = BTreeSet::new();
    let mut excluded_examples: Vec<(u64, String, String)> = Vec::new();
    for ((word, readings), score) in report.scores {
        let entry = SemanticEntry {
            word,
            readings,
            score,
        };
        if entry.word_len() == 2 {
            two_char_before += 1;
            let code = entry.code();
            if fullcodes.contains(&code) {
                excluded += 1;
                collided_codes.insert(code.clone());
                excluded_examples.push((entry.score, entry.word.clone(), code));
                continue;
            }
        }
        pools[entry.word_len() - 2].push(entry);
    }

    eprintln!("二字词 semantic entries(collision 过滤前): {two_char_before}");
    eprintln!("排除的二字词 semantic entries: {excluded}");
    eprintln!(
        "剩余合法二字词 semantic entries(top-N 前): {}",
        two_char_before - excluded
    );
    eprintln!("冲突的 distinct 全码数: {}", collided_codes.len());
    excluded_examples.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    eprintln!("被排除的高频示例(前 15):");
    for (score, word, code) in excluded_examples.iter().take(15) {
        eprintln!("  排除: {word}\t{code}\t{score}");
    }

    // 各词长独立 top-N 选择;合法候选不足目标即失败。
    let mut selected: Vec<SemanticEntry> = Vec::new();
    for (index, &(len, target)) in TARGETS.iter().enumerate() {
        let mut pool = std::mem::take(&mut pools[index]);
        eprintln!("{len} 字词: 合法候选 {} 条,目标 {target} 条", pool.len());
        if pool.len() < target {
            eprintln!("错误: {len} 字合法候选不足目标 {target},不输出缩水数据");
            return ExitCode::FAILURE;
        }
        pool.sort_by(SemanticEntry::selection_cmp);
        pool.truncate(target);
        selected.extend(pool);
    }

    // canonical serialization:词长升序 → 词 Unicode 升序 → 读音序列升序。
    selected.sort_by(|a, b| {
        a.word_len()
            .cmp(&b.word_len())
            .then_with(|| a.word.cmp(&b.word))
            .then_with(|| a.readings.cmp(&b.readings))
    });

    print!("{HEADER}");
    println!("# rows: {}", selected.len());
    for entry in &selected {
        let readings = entry
            .readings
            .iter()
            .map(|reading| reading.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        println!("{}\t{}\t{}", entry.word, readings, entry.score);
    }
    eprintln!("输出 semantic entries: {}", selected.len());
    ExitCode::SUCCESS
}
