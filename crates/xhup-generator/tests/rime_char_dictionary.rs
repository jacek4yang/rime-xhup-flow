//! 生成的固定层静态单字 Rime 词典(2/3/4 码,显式权重)的集成测试:
//! 仅解析内存中的生成结果,不读取任何既有 Rime 词典文件。

use std::collections::{BTreeMap, BTreeSet};

use xhup_generator::generate_rime_char_dictionary;

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
                assert!(weight > 0, "权重应为正数: {line:?}");
                rows.push((zi, code.to_string(), weight));
            }
        }
    }
    (header, rows)
}

/// 提取某字在数据行中的静态码集合(2/3/4 码,忽略权重)。
fn codes_of(rows: &[DictRow], zi: char) -> BTreeSet<&str> {
    rows.iter()
        .filter(|(z, _, _)| *z == zi)
        .map(|(_, code, _)| code.as_str())
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
    assert_eq!(rows.len(), 26753);
    for (len, expected) in [(2, 8573), (3, 9022), (4, 9158)] {
        assert_eq!(
            rows.iter().filter(|(_, code, _)| code.len() == len).count(),
            expected,
            "{len} 码行数"
        );
    }
    let hanzi: BTreeSet<char> = rows.iter().map(|(zi, _, _)| *zi).collect();
    assert_eq!(hanzi.len(), 8103);
    for (len, expected) in [(2, 405), (3, 4812), (4, 8416)] {
        let codes: BTreeSet<&str> = rows
            .iter()
            .filter(|(_, code, _)| code.len() == len)
            .map(|(_, code, _)| code.as_str())
            .collect();
        assert_eq!(codes.len(), expected, "{len} 码 distinct 数");
    }
    let pairs: BTreeSet<(&char, &String)> = rows.iter().map(|(zi, code, _)| (zi, code)).collect();
    assert_eq!(pairs.len(), rows.len(), "无重复 (汉字, 码) 行");
}

#[test]
fn codes_are_lowercase_letters_only() {
    let (_, rows) = parse_dictionary(&generate_rime_char_dictionary());
    for (_, code, _) in &rows {
        assert!(
            code.bytes().all(|byte| byte.is_ascii_lowercase()),
            "码仅含 a-z: {code}"
        );
        assert!(matches!(code.len(), 2..=4), "码长仅 2/3/4: {code}");
    }
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
fn serialization_order_and_anchors() {
    let (_, rows) = parse_dictionary(&generate_rime_char_dictionary());
    // 序列化顺序:码长升序、码字典序升序、权重降序;同码权重唯一故为严格全序。
    for pair in rows.windows(2) {
        let a = (pair[0].1.len(), pair[0].1.as_str(), u32::MAX - pair[0].2);
        let b = (pair[1].1.len(), pair[1].1.as_str(), u32::MAX - pair[1].2);
        assert!(a < b, "序列化顺序应严格递增: {:?} < {:?}", pair[0], pair[1]);
    }
    // 序列化锚点(文件组织,不承担排名语义;排名见权重列)。
    let expected_prefix = [
        ('啊', "aa", 6),
        ('阿', "aa", 5),
        ('锕', "aa", 4),
        ('嗄', "aa", 3),
        ('腌', "aa", 2),
        ('吖', "aa", 1),
    ];
    for (row, expected) in rows.iter().zip(expected_prefix) {
        assert_eq!(row.0, expected.0);
        assert_eq!(row.1, expected.1);
        assert_eq!(row.2, expected.2);
    }
    let last = rows.last().expect("词典非空");
    assert_eq!(last.0, '鲰');
    assert_eq!(last.1, "zzyy");
    assert_eq!(last.2, 1);
}

#[test]
fn same_code_weights_are_unique_and_descend_in_file_order() {
    let (_, rows) = parse_dictionary(&generate_rime_char_dictionary());
    let mut by_code: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
    for (_, code, weight) in &rows {
        by_code.entry(code.as_str()).or_default().push(*weight);
    }
    for (code, weights) in &by_code {
        let unique: BTreeSet<u32> = weights.iter().copied().collect();
        assert_eq!(unique.len(), weights.len(), "{code} 同码权重应唯一");
        assert!(
            weights.windows(2).all(|w| w[0] > w[1]),
            "{code} 文件内同码权重应严格降序"
        );
        assert_eq!(
            weights[0] as usize,
            weights.len(),
            "{code} 组首权重 = 组大小"
        );
        assert_eq!(*weights.last().unwrap(), 1, "{code} 组末权重 = 1");
    }
}

#[test]
fn fanout_sentinels() {
    let (_, rows) = parse_dictionary(&generate_rime_char_dictionary());
    for (code, expected) in [("yi", 136), ("jid", 14), ("jumk", 5)] {
        assert_eq!(
            rows.iter().filter(|(_, c, _)| c == code).count(),
            expected,
            "{code} 扇出"
        );
    }
}

#[test]
fn collision_group_ranking_sentinels() {
    let (_, rows) = parse_dictionary(&generate_rime_char_dictionary());
    // jumk:万象分数 橘 > 桔 > 驹 > 椐 > 枸
    let jumk: Vec<(char, u32)> = rows
        .iter()
        .filter(|(_, code, _)| code == "jumk")
        .map(|(zi, _, weight)| (*zi, *weight))
        .collect();
    assert_eq!(
        jumk,
        [('橘', 5), ('桔', 4), ('驹', 3), ('椐', 2), ('枸', 1)]
    );
    // yi 组首:万象分数最高的是「以」(136),「一」次之(135)
    let yi: Vec<(char, u32)> = rows
        .iter()
        .filter(|(_, code, _)| code == "yi")
        .map(|(zi, _, weight)| (*zi, *weight))
        .collect();
    assert_eq!(yi[0], ('以', 136));
    assert_eq!(yi[1], ('一', 135));
}

#[test]
fn sentinel_code_sets() {
    let (_, rows) = parse_dictionary(&generate_rime_char_dictionary());
    assert_eq!(
        codes_of(&rows, '啊'),
        BTreeSet::from(["aa", "aak", "aakd", "aakk"])
    );
    assert_eq!(
        codes_of(&rows, '阿'),
        BTreeSet::from(["aa", "ee", "aae", "eee", "aaed", "aaek", "eeed", "eeek"])
    );
    assert_eq!(
        codes_of(&rows, '贯'),
        BTreeSet::from(["gr", "grg", "grgr", "grt", "grtr", "grv", "grvr"])
    );
    assert_eq!(
        codes_of(&rows, '欻'),
        BTreeSet::from(["ix", "xu", "ixh", "xuh", "ixhr", "xuhr"])
    );
    assert_eq!(
        codes_of(&rows, '行'),
        BTreeSet::from([
            "hg", "hh", "xk", "hgi", "hhi", "xki", "hgii", "hhii", "xkii"
        ])
    );
    assert_eq!(
        codes_of(&rows, '长'),
        BTreeSet::from(["ih", "vh", "ihp", "vhp", "ihpn", "vhpn"])
    );
}

#[test]
fn ge_lo_luo_collapse_deduplicates_generically() {
    let (_, rows) = parse_dictionary(&generate_rime_char_dictionary());
    // 「咯」的 lo/luo 归一到同一双拼码,全部层级通用去重。
    assert_eq!(
        codes_of(&rows, '咯'),
        BTreeSet::from([
            "ge", "ka", "lo", "gek", "kak", "lok", "gekk", "kakk", "lokk"
        ])
    );
}

#[test]
fn zero_encodable_reading_hanzi_are_absent() {
    let (_, rows) = parse_dictionary(&generate_rime_char_dictionary());
    // 「呣」「嗯」是合法规范汉字且有规范形码,但无 XHUP 可编码规范读音,
    // 因此没有任何静态码条目,不回退到旧词典的兼容音码。
    assert!(codes_of(&rows, '呣').is_empty());
    assert!(codes_of(&rows, '嗯').is_empty());
}

#[test]
fn generation_is_byte_reproducible() {
    let first = generate_rime_char_dictionary();
    let second = generate_rime_char_dictionary();
    assert_eq!(first.as_bytes(), second.as_bytes());
}
