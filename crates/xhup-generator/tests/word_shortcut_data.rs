//! canonical 词语简码数据(`data/shortcuts/word_zero_regression.tsv`)的
//! 语义测试。解析期的字段级硬不变量(4 字段、规范汉字、模式、机械投影、
//! 唯一性、baseline 不相交)由 `word_shortcuts` 模块在载入时断言;本文件
//! 锁定跨层语义与 production policy 哨兵。

use std::collections::BTreeSet;

use xhup_generator::{
    canonical_char_code_entries, canonical_level1_shortcuts, canonical_word_code_entries,
    canonical_word_shortcut_entries,
};

#[test]
fn full_code_alias_is_preserved() {
    // alias 不替换:每条的 (词, 完整码) 必须仍在固定词层。
    let word_codes: BTreeSet<(String, String)> = canonical_word_code_entries()
        .iter()
        .map(|entry| (entry.word().to_string(), entry.code().to_string()))
        .collect();
    for entry in canonical_word_shortcut_entries() {
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
fn shortcuts_are_disjoint_from_baseline_fixed_codes() {
    // ZERO_REGRESSION 安全性:shortcut 码与 baseline fixed exact code 全量不相交。
    let mut baseline: BTreeSet<String> = BTreeSet::new();
    for entry in canonical_level1_shortcuts() {
        baseline.insert(entry.key().as_char().to_string());
    }
    for entry in canonical_char_code_entries() {
        baseline.insert(entry.code().to_string());
    }
    for entry in canonical_word_code_entries() {
        baseline.insert(entry.code().to_string());
    }
    for entry in canonical_word_shortcut_entries() {
        assert!(
            !baseline.contains(&entry.shortcut_code().to_string()),
            "{} {} 与 baseline fixed 码冲突",
            entry.word(),
            entry.shortcut_code()
        );
    }
}

#[test]
fn word_and_code_are_unique() {
    let mut words = BTreeSet::new();
    let mut codes = BTreeSet::new();
    for entry in canonical_word_shortcut_entries() {
        assert!(words.insert(entry.word()), "词重复: {}", entry.word());
        assert!(
            codes.insert(entry.shortcut_code().to_string()),
            "码重复: {}",
            entry.shortcut_code()
        );
    }
}

#[test]
fn time_word_is_not_a_shortcut() {
    // 「时间」的 uij/ujm 在 baseline 中已有 3 码单字(fanout 2/4),不属于
    // ZERO_REGRESSION;它留给未来 FIXED_FIRST 阶段,本层禁止偷跑。
    for entry in canonical_word_shortcut_entries() {
        let code = entry.shortcut_code().to_string();
        assert_ne!(entry.word(), "时间", "「时间」不得出现在本层");
        assert!(code != "uij" && code != "ujm" || entry.word() != "时间");
    }
}

#[test]
fn shortcut_lengths_are_three_to_seven() {
    for entry in canonical_word_shortcut_entries() {
        let length = entry.shortcut_code().len();
        assert!(
            (3..=7).contains(&length),
            "{} {} 长度 {length} 不在 3..=7",
            entry.word(),
            entry.shortcut_code()
        );
    }
}
