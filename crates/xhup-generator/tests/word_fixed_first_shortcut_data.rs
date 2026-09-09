//! canonical FIXED_FIRST 词语简码数据(`data/shortcuts/word_fixed_first.tsv`)
//! 的语义测试。解析期的字段级硬不变量(4 字段、规范汉字、模式、机械投影、
//! 唯一性、PRIMARY 词不相交、命中 baseline fixed 码)由
//! `fixed_first_shortcuts` 模块在载入时断言;本文件锁定跨层语义与
//! production policy 哨兵。

use std::collections::BTreeSet;

use xhup_generator::{
    canonical_fixed_first_shortcut_entries, canonical_level1_shortcuts,
    canonical_primary_shortcut_entries, canonical_word_code_entries,
};

#[test]
fn full_code_alias_is_preserved() {
    // alias 不替换:每条的 (词, 完整码) 必须仍在固定词层。
    let word_codes: BTreeSet<(String, String)> = canonical_word_code_entries()
        .iter()
        .map(|entry| (entry.word().to_string(), entry.code().to_string()))
        .collect();
    for entry in canonical_fixed_first_shortcut_entries() {
        let key = (entry.word().to_string(), entry.full_code().to_string());
        assert!(
            word_codes.contains(&key),
            "{} {} 的完整码关系必须保留",
            entry.word(),
            entry.full_code()
        );
    }
}

#[test]
fn disjoint_from_primary_words() {
    // PRIMARY 与 FIXED_FIRST 共同构成 v2 mapping:一词恰落入其中一层;
    // exact code 可以共享,由 merged ranking 统一仲裁。
    let primary_words: BTreeSet<&str> = canonical_primary_shortcut_entries()
        .iter()
        .map(|entry| entry.word())
        .collect();
    for entry in canonical_fixed_first_shortcut_entries() {
        assert!(
            !primary_words.contains(entry.word()),
            "{} 已持有 PRIMARY 简码",
            entry.word()
        );
    }
}

#[test]
fn shortcuts_collide_with_baseline_fixed_codes() {
    // FIXED_FIRST 语义:shortcut 码必须与 baseline fixed exact code 重码
    // (否则它应属于 PRIMARY 层,而不是本层)。
    let mut baseline: BTreeSet<String> = BTreeSet::new();
    for entry in canonical_level1_shortcuts() {
        baseline.insert(entry.key().as_char().to_string());
    }
    for entry in xhup_generator::canonical_char_code_entries() {
        baseline.insert(entry.code().to_string());
    }
    for entry in canonical_word_code_entries() {
        baseline.insert(entry.code().to_string());
    }
    for entry in canonical_fixed_first_shortcut_entries() {
        assert!(
            baseline.contains(&entry.shortcut_code().to_string()),
            "{} {} 未命中 baseline fixed 码,不属于 FIXED_FIRST 层",
            entry.word(),
            entry.shortcut_code()
        );
    }
}

#[test]
fn word_and_code_are_unique() {
    let mut words = BTreeSet::new();
    let mut codes = BTreeSet::new();
    for entry in canonical_fixed_first_shortcut_entries() {
        assert!(words.insert(entry.word()), "词重复: {}", entry.word());
        assert!(
            codes.insert(entry.shortcut_code().to_string()),
            "码重复: {}",
            entry.shortcut_code()
        );
    }
}

#[test]
fn time_word_sentinel_row() {
    // optimizer v2 FIXED_FIRST 的真实选择锁:「时间」以
    // uij(FI)入库,ujm 不得同时存在(一词最多一码)。
    let rows: Vec<_> = canonical_fixed_first_shortcut_entries()
        .iter()
        .filter(|entry| entry.word() == "时间")
        .collect();
    assert_eq!(rows.len(), 1, "「时间」应恰有一条 FIXED_FIRST 简码");
    assert_eq!(rows[0].full_code().to_string(), "uijm");
    assert_eq!(rows[0].shortcut_code().to_string(), "uij");
    assert_eq!(rows[0].mode(), "FI");
}

#[test]
fn shortcut_lengths_are_three_to_five() {
    for entry in canonical_fixed_first_shortcut_entries() {
        let length = entry.shortcut_code().len();
        assert!(
            (3..=5).contains(&length),
            "{} {} 长度 {length} 不在 3..=5",
            entry.word(),
            entry.shortcut_code()
        );
    }
}
