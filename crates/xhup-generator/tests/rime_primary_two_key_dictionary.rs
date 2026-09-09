//! PRIMARY 词典中的二码子集回归:二码不再是独立 production 层,
//! 但 2-key shortcut 必须完整投影并参与 merged ranking。

use std::collections::BTreeMap;

use xhup_generator::{canonical_primary_shortcut_entries, generate_rime_word_shortcut_dictionary};

#[test]
fn every_primary_two_key_entry_is_in_the_unified_dictionary() {
    let rows: BTreeMap<(String, String), u32> = generate_rime_word_shortcut_dictionary()
        .lines()
        .filter(|line| line.contains('\t'))
        .map(|line| {
            let mut fields = line.split('\t');
            let word = fields.next().expect("词").to_string();
            let code = fields.next().expect("码").to_string();
            let weight = fields.next().expect("权重").parse().expect("整数权重");
            assert!(fields.next().is_none(), "应恰为三列: {line}");
            ((word, code), weight)
        })
        .collect();
    let two_key: Vec<_> = canonical_primary_shortcut_entries()
        .iter()
        .filter(|entry| entry.shortcut_code().len() == 2)
        .collect();
    assert_eq!(two_key.len(), 1_261, "selected v2 二码条数冻结");
    for entry in two_key {
        assert_eq!(
            rows.get(&(entry.word().to_string(), entry.shortcut_code().to_string())),
            Some(&entry.rime_weight()),
            "{} 的二码 {} 必须进入统一 PRIMARY 词典",
            entry.word(),
            entry.shortcut_code()
        );
    }
}
