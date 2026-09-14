//! 搜狗细胞词库聚合层提取器。
//!
//! 从本地抓取产物(全拼 Rime 文本,`词<TAB>带调全拼<TAB>分数`)确定性导出
//! `data/words/sogou/` 的分片 TSV 与 `MANIFEST.tsv`。本工具不访问网络;
//! 输出不含时间戳。过滤与序列化规则见 `data/words/sogou/README.md`:
//!
//! - 词全由 `data/hanzi/readings.tsv` 规范 8105 字组成;
//! - 音节数 = 字数,且每音节 ∈ 该字规范读音集合(去声调归一化);
//! - 同 `(词, 读音序列)` 聚合;按词长升序 → 词 Unicode 升序 → 读音序列升序;
//! - 2~4 字词 → `sogou_cell_NN.tsv`(250,000 行/片),5 字及以上 →
//!   `sogou_long_NN.tsv`(参考层,不参与生成期词码投影);
//! - 每分片 SHA-256 记录于 `MANIFEST.tsv`。

#[path = "common/sha256.rs"]
mod sha256;

#[path = "common/wanxiang.rs"]
mod wanxiang;

use sha256::sha256_hex;
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// 与已提交分片保持一致的分片行数上限。
const LINES_PER_SHARD: usize = 250_000;

/// 规范读音表(`data/hanzi/readings.tsv`)经 `include_str!` 嵌入,与生成器
/// 同源;不额外复制第二份字符表。
const READINGS_TSV: &str = include_str!("../../../data/hanzi/readings.tsv");

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("错误: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os();
    let program = args.next().unwrap_or_default();
    let (Some(input_path), Some(output_dir), None) = (args.next(), args.next(), args.next()) else {
        return Err(format!(
            "用法: {} <sogou_rime_full.txt> <output_dir>",
            program.to_string_lossy()
        ));
    };

    let readings = parse_readings(READINGS_TSV)?;
    let input = fs::read_to_string(&input_path)
        .map_err(|err| format!("无法读取输入 {}: {err}", input_path.to_string_lossy()))?;
    if input.is_empty() {
        return Err("输入文件为空".to_string());
    }
    let input_sha256 = sha256_hex(input.as_bytes());

    let mut entries: BTreeMap<(String, Vec<String>), ()> = BTreeMap::new();
    let mut skipped_not_cjk = 0usize;
    let mut skipped_unknown_char = 0usize;
    let mut skipped_reading = 0usize;
    // 输入音节形如 `tuan1`(声调数字后缀);归一化前剥掉尾部数字。
    let tone_digits = ['1', '2', '3', '4', '5'];
    for (index, line) in input.lines().enumerate() {
        let row = index + 1;
        let mut fields = line.split('\t');
        let (Some(word), Some(pinyin), None) = (fields.next(), fields.next(), fields.next()) else {
            return Err(format!("输入第 {row} 行应为两个 TAB 字段: {line:?}"));
        };
        let char_count = word.chars().count();
        if char_count < 2 {
            skipped_not_cjk += 1;
            continue;
        }
        if !word.chars().all(|ch| readings.contains_key(&ch)) {
            skipped_unknown_char += 1;
            continue;
        }
        let syllables: Vec<&str> = pinyin.split_whitespace().collect();
        if syllables.len() != char_count {
            skipped_reading += 1;
            continue;
        }
        let mut normalized = Vec::with_capacity(char_count);
        let mut valid = true;
        for (ch, syllable) in word.chars().zip(&syllables) {
            let bare = syllable.trim_end_matches(tone_digits);
            let Some(one) = wanxiang::normalize_reading(bare) else {
                valid = false;
                break;
            };
            let allowed = &readings[&ch];
            if !allowed.contains(&one) {
                valid = false;
                break;
            }
            normalized.push(one);
        }
        if !valid {
            skipped_reading += 1;
            continue;
        }
        entries.insert((word.to_string(), normalized), ());
    }
    if entries.is_empty() {
        return Err("输入过滤后没有任何合法词条".to_string());
    }

    // 序列化顺序(README 声明): 词长升序 → 词 Unicode 码点升序 → 读音序列升序。
    // 注意 String 的 Ord 是 UTF-8 字节序, 与 README 的「词 Unicode 升序」一致
    // (UTF-8 字节序 == 码点序), 但词长必须显式作为主键。
    let mut sorted: Vec<&(String, Vec<String>)> = entries.keys().collect();
    sorted.sort_by_key(|(word, readings)| (word.chars().count(), word.clone(), readings.clone()));

    let cell: Vec<&(String, Vec<String>)> = sorted
        .iter()
        .copied()
        .filter(|(word, _)| word.chars().count() <= 4)
        .collect();
    let long: Vec<&(String, Vec<String>)> = sorted
        .iter()
        .copied()
        .filter(|(word, _)| word.chars().count() > 4)
        .collect();
    if cell.is_empty() {
        return Err("过滤后没有任何 2~4 字词条".to_string());
    }

    let output = Path::new(&output_dir);
    fs::create_dir_all(output)
        .map_err(|err| format!("无法创建输出目录 {}: {err}", output.display()))?;

    let cell_files = write_shards(output, "sogou_cell", &cell)?;
    let long_files = write_shards(output, "sogou_long", &long)?;

    let mut manifest = String::new();
    manifest.push_str("# 搜狗细胞词库聚合分片清单(sha256 由提取脚本确定性生成)\n");
    manifest.push_str("# columns: 文件 TAB 行数 TAB sha256\n");
    for (name, lines, hash) in cell_files.iter().chain(long_files.iter()) {
        manifest.push_str(&format!("{name}\t{lines}\t{hash}\n"));
    }
    manifest.push_str(&format!(
        "# input_merged_full_sha256: {input_sha256}\n# entries: {} (cell {} / long {})\n# skipped: not-cjk-or-single {skipped_not_cjk} unknown-char {skipped_unknown_char} reading {skipped_reading}\n",
        entries.len(),
        cell.len(),
        long.len(),
    ));
    let manifest_path = output.join("MANIFEST.tsv");
    fs::write(&manifest_path, &manifest)
        .map_err(|err| format!("无法写出 {}: {err}", manifest_path.display()))?;

    Ok(())
}

fn parse_readings(
    text: &'static str,
) -> Result<HashMap<char, std::collections::BTreeSet<String>>, String> {
    let mut map: HashMap<char, std::collections::BTreeSet<String>> = HashMap::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('\t');
        let (Some(ch_field), Some(reading), Some(_role), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return Err(format!("readings.tsv 行应为三个 TAB 字段: {line:?}"));
        };
        let mut chars = ch_field.chars();
        let Some(ch) = chars.next() else {
            return Err("readings.tsv 存在空字符行".to_string());
        };
        if chars.next().is_some() {
            return Err(format!(
                "readings.tsv 字符字段应为一个 Unicode 标量: {ch_field:?}"
            ));
        }
        map.entry(ch).or_default().insert(reading.to_string());
    }
    if map.len() != 8_105 {
        return Err(format!(
            "readings.tsv 应覆盖 8105 个规范字, 实际 {}",
            map.len()
        ));
    }
    Ok(map)
}

fn write_shards(
    output: &Path,
    prefix: &str,
    entries: &[&(String, Vec<String>)],
) -> Result<Vec<(String, usize, String)>, String> {
    let mut written = Vec::new();
    for (shard_index, chunk) in entries.chunks(LINES_PER_SHARD).enumerate() {
        let mut body = String::new();
        for (word, readings) in chunk {
            body.push_str(word);
            body.push('\t');
            body.push_str(&readings.join(" "));
            body.push('\t');
            body.push_str("1\n");
        }
        let name = format!("{prefix}_{:02}.tsv", shard_index + 1);
        let path: PathBuf = output.join(&name);
        fs::write(&path, &body).map_err(|err| format!("无法写出 {}: {err}", path.display()))?;
        written.push((name, chunk.len(), sha256_hex(body.as_bytes())));
    }
    eprintln!("{prefix}: {} 片, {} 条", written.len(), entries.len());
    Ok(written)
}
