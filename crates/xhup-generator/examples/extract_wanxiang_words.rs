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
//! collision policy:二字词的 4 键码可能与规范单字全码碰撞。**碰撞不排除
//! 词语**(词汇存在性不因码碰撞被剥夺);碰撞码的词/字候选次序由生成器
//! `merged_ranking` 按同源频率证据跨表仲裁。本工具仅统计碰撞规模供审计。
//!
//! shortcut protection:凡被 production canonical v2 简码引用的
//! `(词, 完整码)` 不受 top-N 频率截断影响——简码的资格语义要求宿主词在
//! 固定词层可达;必要时从该词长池的频率尾部逐出等量未受保护条目腾位。
//! 受保护条目在源数据中无匹配 semantic entry 时失败(简码悬空),不静默放过。

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::process::ExitCode;

use xhup_core::{HanziReading, XhupHanzi};
use xhup_generator::{canonical_char_entries, canonical_word_code_entries};

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
# collision_policy: 二字词 4 键码与规范单字全码碰撞时不删除任何 semantic
#   entry;碰撞只影响候选排序,由生成器 merged_ranking 按同源频率证据仲裁
# shortcut_protection: 被规范简码数据(data/shortcuts/*.tsv)引用的
#   (词, 完整码) 与 FIXED_FIRST shortcut 目标码上的词层占用者不受
#   top-N 截断影响,必然入选;必要时从该词长池频率尾部逐出等量未受
#   保护条目腾位;受保护 (词, 完整码) 无源数据匹配时失败
# selection: 各词长独立按 (分数降序, 词 Unicode 升序, 读音序列升序) 取
#   前 50000 / 30000 / 20000 条(含保护递补);合法候选不足目标即失败,不静默缩水
# serialization: 词长升序 → 词 Unicode 升序 → 读音序列升序
";

const EXTENDED_HEADER: &str = "\
# XHUP Flow 扩展词语数据:万象 / RIME-LMDG 基础词库的次级高质量层\n\
# source_repo: amzxyz/rime-wanxiang\n\
# source_commit: 4618d67a978ff4f41b165c10b35558d38e333ab1\n\
# source_path: dicts/jichu.dict.yaml\n\
# source_blob_sha: a0f66e2fc6130f3f1c9b2e5109644c8b893477b0\n\
# source_license: CC-BY-4.0\n\
# selection: 对各词长应用 hot 层完整选择/简码保护后，保留其余全部通过\n\
#   canonical 校验的 semantic entry；不再用另一个 Top-N 截断词汇可达性\n\
# role: pinned 万象词汇证据的完整 secondary tier；真正不存在于来源中的合法\n\
#   组合仍由 open composer 可达\n\
# serialization: 词长升序 → 词 Unicode 升序 → 读音序列升序\n";

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

/// shortcut protection 的输入:宿主词关系 + FIXED_FIRST 目标码。
struct ShortcutProtection {
    /// 被简码引用的 `(词, 完整码)`:宿主词必须留在固定词层。
    word_codes: BTreeSet<(String, String)>,
    /// FIXED_FIRST shortcut 命中的目标码:这些码上的词层占用者必须保留,
    /// 否则 shortcut 失去 baseline 命中而悬空。
    target_codes: BTreeSet<String>,
}

/// 由 generator 已校验的 PRIMARY + FIXED_FIRST 投影构造保护集。
fn load_shortcut_protection() -> ShortcutProtection {
    let mut protection = ShortcutProtection {
        word_codes: BTreeSet::new(),
        target_codes: BTreeSet::new(),
    };
    for entry in xhup_generator::canonical_primary_shortcut_entries() {
        protection
            .word_codes
            .insert((entry.word().to_string(), entry.full_code().to_string()));
    }
    for entry in xhup_generator::canonical_fixed_first_shortcut_entries() {
        protection
            .word_codes
            .insert((entry.word().to_string(), entry.full_code().to_string()));
        protection
            .target_codes
            .insert(entry.shortcut_code().to_string());
    }
    protection
}

fn main() -> ExitCode {
    let mut args = env::args_os();
    let program = args.next().unwrap_or_default();
    let Some(path) = args.next() else {
        eprintln!(
            "用法: {} <本地 jichu.dict.yaml 路径> [--extended]",
            program.to_string_lossy()
        );
        return ExitCode::from(2);
    };
    let extended = match args.next() {
        None => false,
        Some(flag) if flag == "--extended" => true,
        Some(_) => {
            eprintln!("可选第二参数只能是 --extended");
            return ExitCode::from(2);
        }
    };
    if args.next().is_some() {
        eprintln!("参数过多");
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

    // 规范单字全码集:二字词碰撞统计的基准(复用公共推导,无第二份实现)。
    // 仅审计,不过滤:碰撞的词/字共存语义见模块头 collision policy。
    let fullcodes: BTreeSet<String> = canonical_char_entries()
        .iter()
        .map(|entry| entry.code().to_string())
        .collect();
    eprintln!("规范单字全码 distinct 数: {}", fullcodes.len());

    // 按词长分池(不做碰撞过滤);同时统计碰撞规模供审计。
    let mut pools: [Vec<SemanticEntry>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut two_char_total = 0usize;
    let mut collided = 0usize;
    let mut collided_codes: BTreeSet<String> = BTreeSet::new();
    let mut collided_examples: Vec<(u64, String, String)> = Vec::new();
    for ((word, readings), score) in report.scores {
        let entry = SemanticEntry {
            word,
            readings,
            score,
        };
        if entry.word_len() == 2 {
            two_char_total += 1;
            let code = entry.code();
            if fullcodes.contains(&code) {
                collided += 1;
                collided_codes.insert(code.clone());
                collided_examples.push((entry.score, entry.word.clone(), code));
            }
        }
        pools[entry.word_len() - 2].push(entry);
    }

    eprintln!("二字词 semantic entries: {two_char_total}");
    eprintln!("其中与单字全码碰撞(保留,由 merged_ranking 仲裁排序): {collided}");
    eprintln!("碰撞的 distinct 全码数: {}", collided_codes.len());
    collided_examples.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    eprintln!("碰撞的高频示例(前 15):");
    for (score, word, code) in collided_examples.iter().take(15) {
        eprintln!("  共存: {word}\t{code}\t{score}");
    }

    // 各词长独立 top-N 选择;合法候选不足目标即失败。
    // 简码保护层:被规范简码引用的 (词, 完整码) 与 FIXED_FIRST 目标码上的
    // 词层占用者必然入选,必要时从频率尾部逐出等量未受保护条目;受保护
    // (词, 完整码) 在池中无匹配即失败(简码悬空)。
    let protection = load_shortcut_protection();
    let committed_hot_relations: BTreeSet<(String, String)> = canonical_word_code_entries()
        .into_iter()
        .map(|entry| (entry.word().to_string(), entry.code().to_string()))
        .collect();
    eprintln!(
        "简码保护:宿主 (词, 完整码) {} 条,FIXED_FIRST 目标码 {} 个",
        protection.word_codes.len(),
        protection.target_codes.len()
    );
    let mut selected: Vec<SemanticEntry> = Vec::new();
    let mut extended_selected: Vec<SemanticEntry> = Vec::new();
    for (index, &(len, target)) in TARGETS.iter().enumerate() {
        let mut pool = std::mem::take(&mut pools[index]);
        eprintln!("{len} 字词: 合法候选 {} 条,目标 {target} 条", pool.len());
        if pool.len() < target {
            eprintln!("错误: {len} 字合法候选不足目标 {target},不输出缩水数据");
            return ExitCode::FAILURE;
        }
        pool.sort_by(SemanticEntry::selection_cmp);

        // 受保护条目的池内下标;同时校验所有该词长的受保护关系均有匹配。
        let mut protected_idx: Vec<usize> = Vec::new();
        for (i, entry) in pool.iter().enumerate() {
            let code = entry.code();
            if protection
                .word_codes
                .contains(&(entry.word.clone(), code.clone()))
                || protection.target_codes.contains(&code)
            {
                protected_idx.push(i);
            }
        }
        for pair in &protection.word_codes {
            let char_count = pair.0.chars().count();
            if char_count != len {
                continue;
            }
            let found = pool
                .iter()
                .any(|entry| entry.word == pair.0 && entry.code() == pair.1);
            if !found {
                eprintln!(
                    "错误: 简码引用的 (词, 完整码) 在源数据中无匹配 semantic entry(简码悬空): {} {}",
                    pair.0, pair.1
                );
                return ExitCode::FAILURE;
            }
        }

        let mut chosen = vec![false; pool.len()];
        for flag in chosen.iter_mut().take(target) {
            *flag = true;
        }
        let protected_set: BTreeSet<usize> = protected_idx.iter().copied().collect();
        let mut reinstated = 0usize;
        let mut tail = target;
        for &i in &protected_idx {
            if chosen[i] {
                continue;
            }
            // 从已选尾部向下找第一个未受保护条目逐出。
            loop {
                if tail == 0 {
                    eprintln!("错误: {len} 字词受保护条目数超过目标 {target},无法腾位");
                    return ExitCode::FAILURE;
                }
                tail -= 1;
                if chosen[tail] && !protected_set.contains(&tail) {
                    chosen[tail] = false;
                    break;
                }
            }
            chosen[i] = true;
            reinstated += 1;
        }
        eprintln!("{len} 字词: 保护递补 {reinstated} 条(逐出等量尾部条目)");

        let mut extended_count = 0usize;
        for (entry, keep) in pool.into_iter().zip(chosen) {
            if keep {
                selected.push(entry);
            } else if !committed_hot_relations.contains(&(entry.word.clone(), entry.code())) {
                extended_selected.push(entry);
                extended_count += 1;
            }
        }
        eprintln!("{len} 字词: extended 完整尾部 {extended_count} 条");
    }

    // canonical serialization:词长升序 → 词 Unicode 升序 → 读音序列升序。
    selected.sort_by(|a, b| {
        a.word_len()
            .cmp(&b.word_len())
            .then_with(|| a.word.cmp(&b.word))
            .then_with(|| a.readings.cmp(&b.readings))
    });

    extended_selected.sort_by(|a, b| {
        a.word_len()
            .cmp(&b.word_len())
            .then_with(|| a.word.cmp(&b.word))
            .then_with(|| a.readings.cmp(&b.readings))
    });

    let output = if extended {
        &extended_selected
    } else {
        &selected
    };
    print!("{}", if extended { EXTENDED_HEADER } else { HEADER });
    println!("# rows: {}", output.len());
    for entry in output {
        let readings = entry
            .readings
            .iter()
            .map(|reading| reading.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        println!("{}\t{}\t{}", entry.word, readings, entry.score);
    }
    eprintln!(
        "输出 {} semantic entries: {}",
        if extended { "extended" } else { "hot" },
        output.len()
    );
    ExitCode::SUCCESS
}
