//! canonical 二码零冲突词语简码数据(`data/shortcuts/
//! word_two_key_zero_regression.tsv`)的语义测试。字段级硬不变量
//! (4 字段、恰 2 字规范汉字、II 模式、机械投影、唯一性、空码、与
//! ZR/FF 词码不相交)由 `two_key_shortcuts` 模块载入时断言;本文件
//! 锁定跨层语义与 production policy 哨兵。

use std::collections::BTreeSet;

use xhup_generator::{
    canonical_char_code_entries, canonical_fixed_first_shortcut_entries,
    canonical_level1_shortcuts, canonical_two_key_shortcut_entries, canonical_word_code_entries,
    canonical_word_shortcut_entries,
};

#[test]
fn full_code_alias_is_preserved() {
    // alias 不替换:每条的 (词, 完整码) 必须仍在固定词层。
    let word_codes: BTreeSet<(String, String)> = canonical_word_code_entries()
        .iter()
        .map(|entry| (entry.word().to_string(), entry.code().to_string()))
        .collect();
    for entry in canonical_two_key_shortcut_entries() {
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
fn disjoint_from_existing_production_words_and_codes() {
    // 一词最多一条简码:二码层词不得持有 ZR/FF 简码;码全量不相交。
    let existing_words: BTreeSet<&str> = canonical_word_shortcut_entries()
        .iter()
        .map(|entry| entry.word())
        .chain(
            canonical_fixed_first_shortcut_entries()
                .iter()
                .map(|entry| entry.word()),
        )
        .collect();
    let existing_codes: BTreeSet<String> = canonical_word_shortcut_entries()
        .iter()
        .map(|entry| entry.shortcut_code().to_string())
        .chain(
            canonical_fixed_first_shortcut_entries()
                .iter()
                .map(|entry| entry.shortcut_code().to_string()),
        )
        .collect();
    for entry in canonical_two_key_shortcut_entries() {
        assert!(
            !existing_words.contains(entry.word()),
            "{} 已持有既有 production 简码",
            entry.word()
        );
        assert!(
            !existing_codes.contains(&entry.shortcut_code().to_string()),
            "{} {} 与既有 production 码冲突",
            entry.word(),
            entry.shortcut_code()
        );
    }
}

#[test]
fn shortcuts_are_genuinely_empty_codes() {
    // 二码零冲突语义:每个 shortcut 码必须与 baseline fixed exact-code
    // 集合(一级简码 + 单字 2/3/4 码 + 固定词 4/6/8 键)完全不相交
    //(generator 独立重验,与解析器内断言同一语义的跨层锁定)。
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
    for entry in canonical_two_key_shortcut_entries() {
        assert_eq!(entry.mode(), "II", "模式必须为 II");
        let shortcut = entry.shortcut_code().to_string();
        assert_eq!(shortcut.chars().count(), 2, "简码必须 2 键");
        assert!(
            !baseline.contains(&shortcut),
            "{} {} 未命中空码,不属于二码零冲突层",
            entry.word(),
            shortcut
        );
        // 机械 II 投影:两字双拼首键。
        let full = entry.full_code().to_string();
        let chars: Vec<char> = full.chars().collect();
        let initials: String = [chars[0], chars[2]].iter().collect();
        assert_eq!(shortcut, initials, "{} II 投影不一致", entry.word());
    }
}

#[test]
fn word_and_code_are_unique() {
    let mut words = BTreeSet::new();
    let mut codes = BTreeSet::new();
    for entry in canonical_two_key_shortcut_entries() {
        assert!(words.insert(entry.word()), "词重复: {}", entry.word());
        assert!(
            codes.insert(entry.shortcut_code().to_string()),
            "码重复: {}",
            entry.shortcut_code()
        );
    }
}

#[test]
fn time_word_is_not_in_two_key_layer() {
    // 「时间」的 uj 是占用码(fanout 41):必须结构性缺席二码层。
    assert!(
        !canonical_two_key_shortcut_entries()
            .iter()
            .any(|entry| entry.word() == "时间"),
        "时间不得进入二码零冲突层(uj 为占用码)"
    );
    // uj 码本身也不得被任何词使用。
    assert!(
        !canonical_two_key_shortcut_entries()
            .iter()
            .any(|entry| entry.shortcut_code().to_string() == "uj"),
        "uj 是占用码,不得出现在二码零冲突层"
    );
}
