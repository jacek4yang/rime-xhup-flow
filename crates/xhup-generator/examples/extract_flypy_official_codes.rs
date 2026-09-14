//! 小鹤官网权威字形数据提取器。
//!
//! 从本地 pinned `ixdata.json`(官网查形页数据集)确定性导出
//! `data/xhup/flypy_official_char_codes.tsv`。本工具不访问网络;输出不含
//! 时间戳。输入为官网页面原始 payload(数字键名 JSON),本工具负责:
//! 数字键名 → 语义字段映射、字 Unicode 码点升序、TSV 序列化与 payload
//! SHA-256 记录。`-`/`*`/`+` 页面标记保真保留,不做语言学解释。

#[path = "common/sha256.rs"]
mod sha256;

use sha256::sha256_hex;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Write;
use std::process::ExitCode;

use serde_json::Value;

const FIELD_CODE: &str = "1";
const FIELD_STRUCTURE: &str = "2";
const FIELD_COMPONENTS: &str = "3";
const FIELD_PINYIN: &str = "4";

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
    let (Some(ixdata_path), None) = (args.next(), args.next()) else {
        return Err(format!(
            "用法: {} <pinned ixdata.json>",
            program.to_string_lossy()
        ));
    };

    let raw = fs::read(&ixdata_path)
        .map_err(|err| format!("无法读取 {}: {err}", ixdata_path.to_string_lossy()))?;
    let payload_sha256 = sha256_hex(&raw);
    let decoded: BTreeMap<String, Value> =
        serde_json::from_slice(&raw).map_err(|err| format!("payload 不是合法 JSON: {err}"))?;
    if decoded.is_empty() {
        return Err("payload 不含任何字符条目".to_string());
    }

    let mut out = String::new();
    out.push_str("# XHUP Flow 官网权威字形数据:小鹤音形官网查形数据集(ixdata.json)确定性快照\n");
    out.push_str("# source: https://www.flypy.cc/ix/ (官方查形页;数据集随页面 ixdata.json 分发)\n");
    out.push_str(&format!("# source_payload_sha256: {payload_sha256}\n"));
    out.push_str("# source_note: 官网未声明数据许可;所有者决策作为权威参照层入库(见 NOTICE.md 与 docs/data-pipeline.md)\n");
    out.push_str("# page_marker_semantics: 音码/形码后缀 `-`=出简让全(次读音); `*`=生僻音/替代码; `+`=表外字(多为方言字);与 official_char_code_oracle.tsv 语义一致\n");
    out.push_str("# columns: 字 TAB 全码 TAB 二级拆分 TAB 三级拆分 TAB 读音\n");
    out.push_str("#   全码 = 每读音一个 sound2+shape2 码, 空格分隔;标记保真保留,消费方按需剥离\n");
    out.push_str(
        "#   读音 = 官网「拼音」原文(带声调, 空格分隔多读音);与 data/hanzi/readings.tsv 正交\n",
    );
    out.push_str("# serialization: 字 Unicode 码点升序\n");
    out.push_str(&format!("# rows: {}\n", decoded.len()));

    for (character, fields) in &decoded {
        let obj = fields
            .as_object()
            .ok_or_else(|| format!("字符 {character} 的条目应为对象, 实际为 {fields:?}"))?;
        let code = required_text(obj, FIELD_CODE, character, "全码")?;
        let structure = required_text(obj, FIELD_STRUCTURE, character, "二级拆分")?;
        let components = required_text(obj, FIELD_COMPONENTS, character, "三级拆分")?;
        let pinyin = required_text(obj, FIELD_PINYIN, character, "读音")?;
        for field in [&code, &structure, &components, &pinyin] {
            if field.contains('\t') || field.contains('\n') || field.contains('\r') {
                return Err(format!("字符 {character} 的字段含 TAB/换行: {field:?}"));
            }
        }
        out.push_str(character);
        out.push('\t');
        out.push_str(code);
        out.push('\t');
        out.push_str(structure);
        out.push('\t');
        out.push_str(components);
        out.push('\t');
        out.push_str(pinyin);
        out.push('\n');
    }

    std::io::stdout()
        .write_all(out.as_bytes())
        .map_err(|err| format!("写出失败: {err}"))?;
    Ok(())
}

fn required_text<'a>(
    obj: &'a serde_json::Map<String, Value>,
    key: &str,
    character: &str,
    name: &str,
) -> Result<&'a str, String> {
    obj.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("字符 {character} 缺少{name}字段({key})"))
}
