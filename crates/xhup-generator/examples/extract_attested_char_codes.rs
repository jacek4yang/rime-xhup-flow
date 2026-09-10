//! XHUP 事实字符编码提取器。
//!
//! 从本地 pinned `flypy_chars.dict.yaml` 与仓库内的小鹤官网 oracle 合并生产
//! `data/xhup/attested_char_codes.tsv`。本工具不访问网络；输出不含时间戳。

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::process::ExitCode;

const HEADER: &str = "\
# XHUP Flow attested input character codes (generated; do not edit by hand)\n\
# schema: character<TAB>sound_code<TAB>shape_code<TAB>source_weight<TAB>source<TAB>status\n\
# historical_source_repo: boomker/rime-fast-xhup\n\
# historical_source_commit: 308d6d29c5a612fec7282923e49d3cd5453bab48\n\
# historical_source_path: cn_dicts/flypy_chars.dict.yaml\n\
# historical_source_blob: 3c2773335c8108bbe896b9af588d619e22045132\n\
# historical_source_raw_sha256: 5eeda7a9976cf7d8ed1bc487bdccc02d7f423425515d814309328ff61b6b4291\n\
# historical_source_license: LGPL-3.0\n\
# official_oracle: data/xhup/official_char_code_oracle.tsv\n\
# official_payload_decoded_sha256: d33e6afa77d2586c097876674ad11cb33ea834e455be4bb704e05a9f33ad7dd4\n\
# serialization: character Unicode scalar -> sound_code -> shape_code -> source -> status\n";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Evidence {
    character: char,
    sound: String,
    shape: String,
    weight: u32,
    source: &'static str,
    status: String,
}

fn one_character(field: &str, context: &str) -> Result<char, String> {
    let mut chars = field.chars();
    match (chars.next(), chars.next()) {
        (Some(ch), None) => Ok(ch),
        _ => Err(format!(
            "{context}: 字符字段必须恰好包含一个 Unicode 标量: {field:?}"
        )),
    }
}

fn two_keys(field: &str, context: &str) -> Result<String, String> {
    if field.len() == 2 && field.bytes().all(|byte| byte.is_ascii_lowercase()) {
        Ok(field.to_string())
    } else {
        Err(format!(
            "{context}: 编码必须是两个小写 ASCII 字母: {field:?}"
        ))
    }
}

fn parse_historical(text: &str) -> Result<Vec<Evidence>, String> {
    let mut rows = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let row = index + 1;
        if line.is_empty()
            || line.starts_with('#')
            || line == "---"
            || line == "..."
            || line.starts_with("name:")
            || line.starts_with("version:")
            || line.starts_with("sort:")
        {
            continue;
        }
        let context = format!("historical 第 {row} 行");
        let mut fields = line.split('\t');
        let (Some(character), Some(code), Some(weight), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return Err(format!("{context}: 应为三个 TAB 字段: {line:?}"));
        };
        let Some((sound, shape)) = code.split_once('~') else {
            return Err(format!("{context}: 编码应为 sound~shape: {code:?}"));
        };
        rows.push(Evidence {
            character: one_character(character, &context)?,
            sound: two_keys(sound, &context)?,
            shape: two_keys(shape, &context)?,
            weight: weight
                .parse()
                .map_err(|_| format!("{context}: 权重应为 u32: {weight:?}"))?,
            source: "rime-fast-xhup",
            status: "legacy-compatible".to_string(),
        });
    }
    if rows.is_empty() {
        return Err("historical 输入没有数据行".to_string());
    }
    Ok(rows)
}

fn parse_oracle(text: &str) -> Result<Vec<Evidence>, String> {
    let mut rows = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let row = index + 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let context = format!("oracle 第 {row} 行");
        let mut fields = line.split('\t');
        let (Some(character), Some(sound), Some(shape), Some(status), None) = (
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
        ) else {
            return Err(format!("{context}: 应为四个 TAB 字段: {line:?}"));
        };
        if status.is_empty()
            || !status
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
        {
            return Err(format!("{context}: status 非法: {status:?}"));
        }
        rows.push(Evidence {
            character: one_character(character, &context)?,
            sound: two_keys(sound, &context)?,
            shape: two_keys(shape, &context)?,
            weight: 0,
            source: "flypy-official-ix",
            status: status.to_string(),
        });
    }
    if rows.is_empty() {
        return Err("oracle 输入没有数据行".to_string());
    }
    Ok(rows)
}

fn run() -> Result<(), String> {
    let mut args = env::args_os();
    let program = args.next().unwrap_or_default();
    let (Some(historical_path), Some(oracle_path), None) = (args.next(), args.next(), args.next())
    else {
        return Err(format!(
            "用法: {} <pinned flypy_chars.dict.yaml> <official_char_code_oracle.tsv>",
            program.to_string_lossy()
        ));
    };
    let historical = fs::read_to_string(&historical_path).map_err(|err| {
        format!(
            "无法读取 historical {}: {err}",
            historical_path.to_string_lossy()
        )
    })?;
    let oracle = fs::read_to_string(&oracle_path)
        .map_err(|err| format!("无法读取 oracle {}: {err}", oracle_path.to_string_lossy()))?;

    let historical_rows = parse_historical(&historical)?;
    let oracle_rows = parse_oracle(&oracle)?;
    if historical_rows.len() != 9_792 {
        return Err(format!(
            "pinned historical 数据应有 9792 行，实际 {} 行",
            historical_rows.len()
        ));
    }

    let mut rows: Vec<Evidence> = historical_rows
        .iter()
        .cloned()
        .chain(oracle_rows.iter().cloned())
        .collect();
    rows.sort_by(|a, b| {
        (
            a.character,
            &a.sound,
            &a.shape,
            a.source,
            &a.status,
            a.weight,
        )
            .cmp(&(
                b.character,
                &b.sound,
                &b.shape,
                b.source,
                &b.status,
                b.weight,
            ))
    });
    let unique: BTreeSet<&Evidence> = rows.iter().collect();
    if unique.len() != rows.len() {
        return Err("输入存在完全重复的 evidence 行".to_string());
    }
    let characters: BTreeSet<char> = rows.iter().map(|row| row.character).collect();

    print!("{HEADER}");
    println!("# historical_rows: {}", historical_rows.len());
    println!("# official_oracle_rows: {}", oracle_rows.len());
    println!("# evidence_rows: {}", rows.len());
    println!("# input_characters: {}", characters.len());
    for row in &rows {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            row.character, row.sound, row.shape, row.weight, row.source, row.status
        );
    }
    eprintln!(
        "historical={} official={} evidence={} input_characters={}",
        historical_rows.len(),
        oracle_rows.len(),
        rows.len(),
        characters.len()
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("错误: {err}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsers_fail_loudly_on_malformed_input() {
        assert!(parse_historical("嗯\tenkx\t46\n").is_err());
        assert!(parse_historical("嗯\ten~kx\tnot-a-number\n").is_err());
        assert!(parse_oracle("嗯\ten\tkx\n").is_err());
        assert!(parse_oracle("嗯\te1\tkx\tofficial\n").is_err());
    }

    #[test]
    fn parsers_keep_input_codes_separate_from_readings() {
        let row = parse_historical("嗯\tng~kx\t46\n").unwrap().remove(0);
        assert_eq!(row.sound, "ng");
        assert_eq!(row.shape, "kx");
        assert_eq!(row.weight, 46);
    }
}
