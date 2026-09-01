//! 生成的一级简码 Rime 词典(26 条一键关系,显式权重 1)的集成测试:
//! 仅解析内存中的生成结果,不读取任何既有 Rime 词典文件。

use std::collections::{BTreeMap, BTreeSet};

use xhup_generator::{canonical_char_entries, generate_rime_shortcut_dictionary};

/// 数据行 `(汉字, 码, 权重)`。
type DictRow = (char, String, u32);

/// 解析生成词典:返回 header 字段映射与数据行序列。
fn parse_dictionary(text: &str) -> (BTreeMap<String, String>, Vec<DictRow>) {
    let mut header = BTreeMap::new();
    let mut rows = Vec::new();
    let mut in_header = false;
    for line in text.split('\n') {
        match line {
            "# Rime dictionary" | "# encoding: utf-8" | "" => {}
            "---" => in_header = true,
            "..." => in_header = false,
            _ if in_header => {
                let (key, value) = line.split_once(": ").expect("header 字段格式");
                header.insert(key.to_string(), value.to_string());
            }
            _ => {
                let mut fields = line.split('\t');
                let (Some(zi), Some(code), Some(weight), None) =
                    (fields.next(), fields.next(), fields.next(), fields.next())
                else {
                    panic!("数据行应恰好为三列 `汉字<TAB>码<TAB>权重`: {line:?}");
                };
                let mut chars = zi.chars();
                let zi = chars.next().expect("汉字字段非空");
                assert!(chars.next().is_none(), "汉字字段恰为一个字");
                let weight: u32 = weight.parse().expect("权重应为正 u32");
                rows.push((zi, code.to_string(), weight));
            }
        }
    }
    (header, rows)
}

#[test]
fn header_semantics() {
    let (header, _) = parse_dictionary(&generate_rime_shortcut_dictionary());
    assert_eq!(header.len(), 4);
    assert_eq!(header["name"], "xhup_flow_shortcuts");
    assert_eq!(
        header["version"],
        format!("\"{}\"", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(header["sort"], "by_weight");
    assert_eq!(header["use_preset_vocabulary"], "false");
}

#[test]
fn rows_are_26_unique_single_key_entries_with_unit_weight() {
    let (_, rows) = parse_dictionary(&generate_rime_shortcut_dictionary());
    assert_eq!(rows.len(), 26, "恰好 26 行");
    let codes: BTreeSet<&str> = rows.iter().map(|(_, code, _)| code.as_str()).collect();
    assert_eq!(codes.len(), 26, "26 个 distinct 码");
    let hanzi: BTreeSet<char> = rows.iter().map(|(zi, _, _)| *zi).collect();
    assert_eq!(hanzi.len(), 26, "汉字唯一");
    let pairs: BTreeSet<(char, &str)> = rows
        .iter()
        .map(|(zi, code, _)| (*zi, code.as_str()))
        .collect();
    assert_eq!(pairs.len(), 26, "无重复 (汉字, 键) 行");
    for (zi, code, weight) in &rows {
        assert_eq!(code.len(), 1, "码长恰为 1: {code}");
        assert!(
            code.bytes().all(|b| b.is_ascii_lowercase()),
            "码为小写 a-z: {code}"
        );
        assert_eq!(*weight, 1, "每个一键码恰一个候选,权重恒为 1: {zi} {code}");
    }
}

#[test]
fn serialization_is_qwerty_order() {
    // 行顺序为 canonical 序列化顺序(QWERTY 物理布局),不承担排名语义。
    let (_, rows) = parse_dictionary(&generate_rime_shortcut_dictionary());
    let sequence: String = rows
        .iter()
        .map(|(_, code, _)| code.chars().next().unwrap())
        .collect();
    assert_eq!(sequence, "qwertyuiopasdfghjklzxcvbnm");
}

#[test]
fn shortcut_codes_do_not_collide_with_char_or_word_layers() {
    // 1 键码与单字层(2/3/4 码)、词语层(4/6/8 键)不存在 exact code 冲突;
    // 显式断言:无 1 键规范单字条目。
    let one_key_chars = canonical_char_entries()
        .iter()
        .filter(|entry| entry.code().to_string().len() == 1)
        .count();
    assert_eq!(one_key_chars, 0, "规范单字层不应存在 1 键条目");
}

#[test]
fn output_is_utf8_lf_only_with_single_final_newline() {
    let text = generate_rime_shortcut_dictionary();
    assert!(text.is_char_boundary(text.len()));
    assert!(!text.starts_with('\u{feff}'), "无 BOM");
    assert!(!text.contains('\r'), "LF only");
    assert!(
        text.ends_with('\n') && !text.ends_with("\n\n"),
        "恰好一个末尾换行"
    );
}

#[test]
fn generation_is_byte_reproducible() {
    let first = generate_rime_shortcut_dictionary();
    let second = generate_rime_shortcut_dictionary();
    assert_eq!(first.as_bytes(), second.as_bytes());
}
