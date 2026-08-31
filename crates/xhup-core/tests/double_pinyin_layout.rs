//! `data/double_pinyin/` 规范数据的结构校验与哨兵映射检查。
//!
//! TSV 文件是唯一的映射事实来源;本测试只验证格式、完整性与少量代表性映射,
//! 不在 Rust 代码中复制完整映射表。

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use xhup_core::{DoublePinyinCode, Key};

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/double_pinyin")
}

/// 读取并解析 TSV:纯 LF 行尾,每行恰好两个 TAB 分隔字段,字段非空且无首尾空白。
fn read_rows(name: &str) -> Vec<(String, String)> {
    let path = data_dir().join(name);
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("无法读取 {}: {err}", path.display()));
    assert!(!content.contains('\r'), "{name} 不允许 CR 字符");
    assert!(content.ends_with('\n'), "{name} 应以单个换行结尾");

    let mut rows = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let row = index + 1;
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(fields.len(), 2, "{name} 第 {row} 行应有 2 个字段: {line:?}");
        for field in [fields[0], fields[1]] {
            assert!(!field.is_empty(), "{name} 第 {row} 行字段不能为空");
            assert_eq!(
                field.trim(),
                field,
                "{name} 第 {row} 行字段含首尾空白: {field:?}"
            );
        }
        rows.push((fields[0].to_string(), fields[1].to_string()));
    }
    assert!(!rows.is_empty(), "{name} 不应为空文件");
    rows
}

fn assert_lowercase_ascii(rows: &[(String, String)], name: &str) {
    for (first, second) in rows {
        for field in [first, second] {
            assert!(
                field.bytes().all(|byte| byte.is_ascii_lowercase()),
                "{name} 字段应为小写 ASCII: {field:?}"
            );
        }
    }
}

fn assert_sorted_unique_first_column(rows: &[(String, String)], name: &str) {
    let mut seen = BTreeSet::new();
    for (index, (first, _)) in rows.iter().enumerate() {
        assert!(seen.insert(first), "{name} 首列重复: {first:?}");
        if index > 0 {
            let previous = &rows[index - 1].0;
            assert!(
                previous < first,
                "{name} 首列未按字典序升序:{previous:?} 在 {first:?} 之前"
            );
        }
    }
}

/// 键位列必须是单字符且能通过 `Key` 校验。
fn parse_key(value: &str, name: &str) -> Key {
    let mut chars = value.chars();
    let ch = chars.next().unwrap();
    assert!(chars.next().is_none(), "{name} 键位应为单字符: {value:?}");
    Key::from_char(ch).unwrap_or_else(|_| panic!("{name} 键位非法: {value:?}"))
}

fn lookup<'a>(rows: &'a [(String, String)], first: &str) -> Option<&'a str> {
    rows.iter()
        .find(|(f, _)| f == first)
        .map(|(_, s)| s.as_str())
}

#[test]
fn initials_are_valid_unique_and_complete() {
    let rows = read_rows("initials.tsv");
    assert_eq!(rows.len(), 23, "小鹤声母应有 23 行");
    assert_lowercase_ascii(&rows, "initials.tsv");
    assert_sorted_unique_first_column(&rows, "initials.tsv");

    let mut keys = BTreeSet::new();
    for (_, key) in &rows {
        assert!(
            keys.insert(parse_key(key, "initials.tsv")),
            "initials.tsv 键位重复: {key:?}"
        );
    }

    // 哨兵映射:恒等单声母与三个特殊双声母
    assert_eq!(lookup(&rows, "b"), Some("b"));
    assert_eq!(lookup(&rows, "zh"), Some("v"));
    assert_eq!(lookup(&rows, "ch"), Some("i"));
    assert_eq!(lookup(&rows, "sh"), Some("u"));
}

#[test]
fn finals_are_valid_and_cover_every_key() {
    let rows = read_rows("finals.tsv");
    assert_eq!(rows.len(), 33, "小鹤韵母应有 33 行");
    assert_lowercase_ascii(&rows, "finals.tsv");
    assert_sorted_unique_first_column(&rows, "finals.tsv");

    // 共享键合法(如 ing/uai、ue/ve),不要求键位唯一;
    // 但 26 个字母键都必须至少被一个韵母覆盖。
    let keys: BTreeSet<Key> = rows
        .iter()
        .map(|(_, key)| parse_key(key, "finals.tsv"))
        .collect();
    let all_keys: BTreeSet<Key> = ('a'..='z').map(|ch| Key::from_char(ch).unwrap()).collect();
    assert_eq!(keys, all_keys, "finals.tsv 应覆盖全部 26 个键位");

    // 哨兵映射:简单韵母、共享键、ang、ing
    assert_eq!(lookup(&rows, "a"), Some("a"));
    assert_eq!(lookup(&rows, "ong"), Some("s"));
    assert_eq!(lookup(&rows, "iong"), Some("s"));
    assert_eq!(lookup(&rows, "ang"), Some("h"));
    assert_eq!(lookup(&rows, "ing"), Some("k"));
    assert_eq!(lookup(&rows, "ue"), Some("t"));
    assert_eq!(lookup(&rows, "ve"), Some("t"));
}

#[test]
fn zero_initials_are_valid_with_unique_codes() {
    let rows = read_rows("zero_initials.tsv");
    assert_eq!(rows.len(), 12, "零声母音节应有 12 行");
    assert_lowercase_ascii(&rows, "zero_initials.tsv");
    assert_sorted_unique_first_column(&rows, "zero_initials.tsv");

    let mut codes = BTreeSet::new();
    for (_, code) in &rows {
        let parsed: DoublePinyinCode = code
            .parse()
            .unwrap_or_else(|_| panic!("zero_initials.tsv 编码非法: {code:?}"));
        assert!(codes.insert(parsed), "zero_initials.tsv 编码重复: {code:?}");
    }

    // 哨兵映射:单字母重复、双字母原样、三字母取首字母 + 韵母键
    assert_eq!(lookup(&rows, "a"), Some("aa"));
    assert_eq!(lookup(&rows, "an"), Some("an"));
    assert_eq!(lookup(&rows, "ang"), Some("ah"));
}
