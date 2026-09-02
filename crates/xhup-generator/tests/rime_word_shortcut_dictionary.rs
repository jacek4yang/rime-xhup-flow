//! 词语简码 Rime 词典(`xhup_flow_word_shortcuts.dict.yaml`)的生成测试:
//! 行数、权重、码合法性、唯一性与字节级确定性。

use std::collections::BTreeSet;

use xhup_generator::{canonical_word_shortcut_entries, generate_rime_word_shortcut_dictionary};

/// 词典数据行(跳过头部与 `...` 结束标记)。
fn body_rows(dict: &str) -> Vec<&str> {
    dict.lines()
        .skip_while(|line| *line != "...")
        .skip(1)
        .collect()
}

#[test]
fn rows_match_canonical_entry_count() {
    let dict = generate_rime_word_shortcut_dictionary();
    assert_eq!(
        body_rows(&dict).len(),
        canonical_word_shortcut_entries().len(),
        "词典行数应等于 canonical 生产简码条数"
    );
}

#[test]
fn every_row_is_word_code_weight_one() {
    let dict = generate_rime_word_shortcut_dictionary();
    let canonical: BTreeSet<(String, String)> = canonical_word_shortcut_entries()
        .iter()
        .map(|e| (e.word().to_string(), e.shortcut_code().to_string()))
        .collect();
    let mut seen_words = BTreeSet::new();
    let mut seen_codes = BTreeSet::new();
    for row in body_rows(&dict) {
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 3, "每行 词<TAB>码<TAB>权重: {row}");
        assert_eq!(fields[2], "1", "权重恒为 1: {row}");
        assert!(
            fields[1].chars().all(|c| c.is_ascii_lowercase()),
            "码为纯小写 a-z: {row}"
        );
        assert!((3..=7).contains(&fields[1].len()), "码长度在 3..=7: {row}");
        assert!(
            canonical.contains(&(fields[0].to_string(), fields[1].to_string())),
            "行必须来自 canonical 集合: {row}"
        );
        assert!(seen_words.insert(fields[0]), "词重复: {row}");
        assert!(seen_codes.insert(fields[1]), "码重复: {row}");
    }
}

#[test]
fn dictionary_header_matches_conventions() {
    let dict = generate_rime_word_shortcut_dictionary();
    let expected_version = format!("version: \"{}\"", env!("CARGO_PKG_VERSION"));
    for line in [
        "# Rime dictionary",
        "name: xhup_flow_word_shortcuts",
        expected_version.as_str(),
        "sort: by_weight",
        "use_preset_vocabulary: false",
    ] {
        assert!(dict.contains(line), "词典缺少 `{line}`");
    }
    assert!(!dict.starts_with('\u{feff}'), "无 BOM");
    assert!(!dict.contains('\r'), "LF only");
    assert!(
        dict.ends_with('\n') && !dict.ends_with("\n\n"),
        "恰好一个末尾换行"
    );
}

#[test]
fn generation_is_byte_identical() {
    assert_eq!(
        generate_rime_word_shortcut_dictionary(),
        generate_rime_word_shortcut_dictionary(),
        "两次生成字节级一致"
    );
}
