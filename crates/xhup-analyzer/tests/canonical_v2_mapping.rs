//! selected optimizer v2 dump 与 production canonical 两层的内容承诺。

use std::collections::BTreeMap;

use xhup_analyzer::export_v2::sha256_hex;

const FIXED: &str = include_str!("../../../data/shortcuts/word_fixed_first.tsv");
const PRIMARY: &str = include_str!("../../../data/shortcuts/word_shortcuts_primary.tsv");

fn header<'a>(text: &'a str, prefix: &str) -> &'a str {
    text.lines()
        .find_map(|line| line.strip_prefix(prefix))
        .unwrap_or_else(|| panic!("缺少 provenance 头: {prefix}"))
}

#[test]
fn canonical_layers_equal_the_selected_mapping_content_commitment() {
    assert_eq!(
        header(FIXED, "# input dump sha256: "),
        header(PRIMARY, "# input dump sha256: "),
        "两层必须来自同一个 selected dump"
    );
    let expected = header(FIXED, "# selected mapping sha256: ");
    assert_eq!(
        expected,
        header(PRIMARY, "# selected mapping sha256: "),
        "两层必须携带同一个 selected mapping 内容承诺"
    );

    let mut mapping: BTreeMap<String, (String, usize)> = BTreeMap::new();
    let mut fixed_count = 0usize;
    for line in FIXED.lines().filter(|line| !line.starts_with('#')) {
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(fields.len(), 4, "FIXED_FIRST 应为四列: {line}");
        assert!(
            mapping
                .insert(fields[0].to_string(), (fields[2].to_string(), 1))
                .is_none(),
            "跨层词重复: {}",
            fields[0]
        );
        fixed_count += 1;
    }
    let mut primary_count = 0usize;
    for line in PRIMARY.lines().filter(|line| !line.starts_with('#')) {
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(fields.len(), 4, "PRIMARY 应为四列: {line}");
        let merged_rank = fields[3].parse().expect("merged rank 应为正整数");
        assert!(
            mapping
                .insert(fields[0].to_string(), (fields[1].to_string(), merged_rank),)
                .is_none(),
            "跨层词重复: {}",
            fields[0]
        );
        primary_count += 1;
    }

    assert_eq!(fixed_count, 2_933, "selected FIXED_FIRST 条数");
    assert_eq!(primary_count, 65_909, "selected PRIMARY 条数");
    assert_eq!(mapping.len(), 68_842, "两层并集应完整覆盖 selected mapping");

    let normalized: String = mapping
        .iter()
        .map(|(word, (code, rank))| format!("{word}\t{code}\t{rank}\n"))
        .collect();
    assert_eq!(
        sha256_hex(normalized.as_bytes()),
        expected,
        "canonical 两层重建内容必须与 selected mapping 完全一致"
    );
}
