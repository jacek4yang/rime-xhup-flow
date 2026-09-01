//! 生成的规范单字全码 Rime 词典的集成测试:仅解析内存中的生成结果,
//! 不读取任何既有 Rime 词典文件。

use std::collections::{BTreeMap, BTreeSet};

use xhup_generator::generate_rime_char_dictionary;

/// 解析生成词典:返回 header 字段映射与数据行 `(汉字, 全码)` 序列。
fn parse_dictionary(text: &str) -> (BTreeMap<String, String>, Vec<(char, String)>) {
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
                let (zi, code) = line.split_once('\t').expect("数据行格式");
                let mut chars = zi.chars();
                let zi = chars.next().expect("汉字字段非空");
                assert!(chars.next().is_none(), "汉字字段恰为一个字");
                rows.push((zi, code.to_string()));
            }
        }
    }
    (header, rows)
}

/// 提取某字在数据行中的全码集合。
fn codes_of(rows: &[(char, String)], zi: char) -> BTreeSet<&str> {
    rows.iter()
        .filter(|(z, _)| *z == zi)
        .map(|(_, code)| code.as_str())
        .collect()
}

#[test]
fn header_semantics() {
    let (header, _) = parse_dictionary(&generate_rime_char_dictionary());
    assert_eq!(header.len(), 4);
    assert_eq!(header["name"], "xhup_flow_chars");
    assert_eq!(
        header["version"],
        format!("\"{}\"", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(header["sort"], "by_weight");
    assert_eq!(header["use_preset_vocabulary"], "false");
}

#[test]
fn row_counts_and_uniqueness() {
    let (_, rows) = parse_dictionary(&generate_rime_char_dictionary());
    assert_eq!(rows.len(), 9158);
    let hanzi: BTreeSet<char> = rows.iter().map(|(zi, _)| *zi).collect();
    assert_eq!(hanzi.len(), 8103);
    let codes: BTreeSet<&str> = rows.iter().map(|(_, code)| code.as_str()).collect();
    assert_eq!(codes.len(), 8416);
    let pairs: BTreeSet<(&char, &String)> = rows.iter().map(|(zi, code)| (zi, code)).collect();
    assert_eq!(pairs.len(), rows.len(), "无重复 (汉字, 全码) 行");
}

#[test]
fn output_is_utf8_lf_only_with_single_final_newline() {
    let text = generate_rime_char_dictionary();
    assert!(text.is_char_boundary(text.len()));
    assert!(!text.starts_with('\u{feff}'), "无 BOM");
    assert!(!text.contains('\r'), "LF only");
    assert!(
        text.ends_with('\n') && !text.ends_with("\n\n"),
        "恰好一个末尾换行"
    );
}

#[test]
fn serialization_order_is_deterministic_canonical_order() {
    let (_, rows) = parse_dictionary(&generate_rime_char_dictionary());
    for pair in rows.windows(2) {
        assert!(pair[0] < pair[1], "汉字码点升序、同字内全码升序且无重复");
    }
    // 序列化顺序锚点(不代表候选排序)。
    let expected_prefix = [
        ('㑇', "vzre"),
        ('㑊', "yird"),
        ('㕮', "fukx"),
        ('㘎', "hjkw"),
        ('㙍', "doty"),
    ];
    for (row, expected) in rows.iter().zip(expected_prefix) {
        assert_eq!(row.0, expected.0);
        assert_eq!(row.1, expected.1);
    }
    let last = rows.last().expect("词典非空");
    assert_eq!(last.0, '𬺓');
    assert_eq!(last.1, "iuvr");
}

#[test]
fn sentinel_full_code_sets() {
    let (_, rows) = parse_dictionary(&generate_rime_char_dictionary());
    assert_eq!(codes_of(&rows, '啊'), BTreeSet::from(["aakd", "aakk"]));
    assert_eq!(
        codes_of(&rows, '阿'),
        BTreeSet::from(["aaed", "aaek", "eeed", "eeek"])
    );
    assert_eq!(
        codes_of(&rows, '贯'),
        BTreeSet::from(["grgr", "grtr", "grvr"])
    );
    assert_eq!(codes_of(&rows, '欻'), BTreeSet::from(["ixhr", "xuhr"]));
    assert_eq!(
        codes_of(&rows, '行'),
        BTreeSet::from(["hgii", "hhii", "xkii"])
    );
    assert_eq!(codes_of(&rows, '长'), BTreeSet::from(["ihpn", "vhpn"]));
}

#[test]
fn ge_lo_luo_collapse_deduplicates_generically() {
    let (_, rows) = parse_dictionary(&generate_rime_char_dictionary());
    // 「咯」的 lo/luo 归一到同一双拼码,4 个原始组合去重为 3 行。
    assert_eq!(codes_of(&rows, '咯').len(), 3);
}

#[test]
fn zero_encodable_reading_hanzi_are_absent() {
    let (_, rows) = parse_dictionary(&generate_rime_char_dictionary());
    // 「呣」「嗯」是合法规范汉字且有规范形码,但无 XHUP 可编码规范读音,
    // 因此没有规范全码条目,不回退到旧词典的兼容音码。
    assert!(codes_of(&rows, '呣').is_empty());
    assert!(codes_of(&rows, '嗯').is_empty());
}

#[test]
fn full_code_collisions_across_hanzi_are_preserved() {
    let (_, rows) = parse_dictionary(&generate_rime_char_dictionary());
    let hanzi: BTreeSet<char> = rows
        .iter()
        .filter(|(_, code)| code == "jumk")
        .map(|(zi, _)| *zi)
        .collect();
    assert_eq!(hanzi, BTreeSet::from(['枸', '桔', '椐', '橘', '驹']));
}

#[test]
fn generation_is_byte_reproducible() {
    let first = generate_rime_char_dictionary();
    let second = generate_rime_char_dictionary();
    assert_eq!(first.as_bytes(), second.as_bytes());
}
