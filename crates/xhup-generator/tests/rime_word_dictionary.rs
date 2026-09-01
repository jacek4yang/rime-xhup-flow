//! 生成的固定层静态词语 Rime 词典(2~4 字词 4/6/8 键,显式权重)的集成测试:
//! 仅解析内存中的生成结果,不读取任何既有 Rime 词典文件。

use std::collections::{BTreeMap, BTreeSet};

use xhup_generator::{canonical_char_entries, generate_rime_word_dictionary};

/// 数据行 `(词, 码, 权重)`。
type DictRow = (String, String, u32);

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
                let (Some(word), Some(code), Some(weight), None) =
                    (fields.next(), fields.next(), fields.next(), fields.next())
                else {
                    panic!("数据行应恰好为三列 `词<TAB>码<TAB>权重`: {line:?}");
                };
                let weight: u32 = weight.parse().expect("权重应为正 u32");
                assert!(weight > 0, "权重应为正数: {line:?}");
                rows.push((word.to_string(), code.to_string(), weight));
            }
        }
    }
    (header, rows)
}

/// 某词在数据行中的静态码集合(忽略权重)。
fn codes_of<'a>(rows: &'a [DictRow], word: &str) -> BTreeSet<&'a str> {
    rows.iter()
        .filter(|(w, _, _)| w == word)
        .map(|(_, code, _)| code.as_str())
        .collect()
}

#[test]
fn header_semantics() {
    let (header, _) = parse_dictionary(&generate_rime_word_dictionary());
    assert_eq!(header.len(), 4);
    assert_eq!(header["name"], "xhup_flow_words");
    assert_eq!(
        header["version"],
        format!("\"{}\"", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(header["sort"], "by_weight");
    assert_eq!(header["use_preset_vocabulary"], "false");
}

#[test]
fn row_counts_and_uniqueness() {
    let (_, rows) = parse_dictionary(&generate_rime_word_dictionary());
    assert_eq!(rows.len(), 100_000);
    for (len, expected) in [(4, 50_000), (6, 30_000), (8, 20_000)] {
        assert_eq!(
            rows.iter().filter(|(_, code, _)| code.len() == len).count(),
            expected,
            "{len} 键行数"
        );
    }
    let codes: BTreeSet<&str> = rows.iter().map(|(_, code, _)| code.as_str()).collect();
    assert_eq!(codes.len(), 81_931, "distinct 码数");
    let pairs: BTreeSet<(&String, &String)> =
        rows.iter().map(|(word, code, _)| (word, code)).collect();
    assert_eq!(pairs.len(), rows.len(), "无重复 (词, 码) 行");
}

#[test]
fn codes_are_lowercase_letters_only_and_match_word_length() {
    let (_, rows) = parse_dictionary(&generate_rime_word_dictionary());
    for (word, code, _) in &rows {
        assert!(
            code.bytes().all(|byte| byte.is_ascii_lowercase()),
            "码仅含 a-z: {code}"
        );
        assert!(matches!(code.len(), 4 | 6 | 8), "码长仅 4/6/8: {code}");
        assert_eq!(
            code.len(),
            word.chars().count() * 2,
            "码长 = 字数 × 2: {word} {code}"
        );
    }
}

#[test]
fn two_char_codes_are_disjoint_from_canonical_fullcodes() {
    // P0:最终词词典中的 4 键码绝不占用规范单字全码。
    let fullcodes: BTreeSet<String> = canonical_char_entries()
        .iter()
        .map(|entry| entry.code().to_string())
        .collect();
    let (_, rows) = parse_dictionary(&generate_rime_word_dictionary());
    for (_, code, _) in &rows {
        if code.len() == 4 {
            assert!(!fullcodes.contains(code), "4 键词码 {code} 与规范全码冲突");
        }
    }
}

#[test]
fn output_is_utf8_lf_only_with_single_final_newline() {
    let text = generate_rime_word_dictionary();
    assert!(text.is_char_boundary(text.len()));
    assert!(!text.starts_with('\u{feff}'), "无 BOM");
    assert!(!text.contains('\r'), "LF only");
    assert!(
        text.ends_with('\n') && !text.ends_with("\n\n"),
        "恰好一个末尾换行"
    );
}

#[test]
fn serialization_order_is_strictly_increasing() {
    // 序列化顺序:码长升序、码字典序升序、权重降序、词 Unicode 升序;
    // 同码权重唯一故为严格全序(不承担排名语义;排名见权重列)。
    let (_, rows) = parse_dictionary(&generate_rime_word_dictionary());
    for pair in rows.windows(2) {
        let a = (
            pair[0].1.len(),
            pair[0].1.as_str(),
            u32::MAX - pair[0].2,
            pair[0].0.as_str(),
        );
        let b = (
            pair[1].1.len(),
            pair[1].1.as_str(),
            u32::MAX - pair[1].2,
            pair[1].0.as_str(),
        );
        assert!(a < b, "序列化顺序应严格递增: {:?} < {:?}", pair[0], pair[1]);
    }
}

#[test]
fn same_code_weights_are_unique_and_descend_in_file_order() {
    let (_, rows) = parse_dictionary(&generate_rime_word_dictionary());
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
    let (_, rows) = parse_dictionary(&generate_rime_word_dictionary());
    for (code, expected) in [("yivi", 19), ("uijm", 7), ("keyi", 4)] {
        assert_eq!(
            rows.iter().filter(|(_, c, _)| c == code).count(),
            expected,
            "{code} 扇出"
        );
    }
}

#[test]
fn collision_group_ranking_sentinels() {
    let (_, rows) = parse_dictionary(&generate_rime_word_dictionary());
    // keyi:万象分数 可以 > 刻意 > 可疑 > 科一
    let keyi: Vec<(&str, u32)> = rows
        .iter()
        .filter(|(_, code, _)| code == "keyi")
        .map(|(word, _, weight)| (word.as_str(), *weight))
        .collect();
    assert_eq!(keyi, [("可以", 4), ("刻意", 3), ("可疑", 2), ("科一", 1)]);
    // yivi 组首/组末:一直 19 …… 逸致 1
    let yivi: Vec<(&str, u32)> = rows
        .iter()
        .filter(|(_, code, _)| code == "yivi")
        .map(|(word, _, weight)| (word.as_str(), *weight))
        .collect();
    assert_eq!(yivi.len(), 19);
    assert_eq!(yivi[0], ("一直", 19));
    assert_eq!(yivi[1], ("一只", 18));
    assert_eq!(yivi[18], ("逸致", 1));
}

#[test]
fn sentinel_code_sets() {
    let (_, rows) = parse_dictionary(&generate_rime_word_dictionary());
    // 二字词 4 键
    assert_eq!(codes_of(&rows, "我们"), BTreeSet::from(["womf"]));
    assert_eq!(codes_of(&rows, "中国"), BTreeSet::from(["vsgo"]));
    assert_eq!(codes_of(&rows, "可以"), BTreeSet::from(["keyi"]));
    assert_eq!(codes_of(&rows, "时间"), BTreeSet::from(["uijm"]));
    // 三字词 6 键
    assert_eq!(codes_of(&rows, "输入法"), BTreeSet::from(["uurufa"]));
    assert_eq!(codes_of(&rows, "图书馆"), BTreeSet::from(["tuuugr"]));
    assert_eq!(codes_of(&rows, "科学家"), BTreeSet::from(["kextjx"]));
    // 四字词 8 键
    assert_eq!(codes_of(&rows, "社会主义"), BTreeSet::from(["uehvvuyi"]));
    assert_eq!(codes_of(&rows, "众所周知"), BTreeSet::from(["vssovzvi"]));
    assert_eq!(codes_of(&rows, "各种各样"), BTreeSet::from(["gevsgeyh"]));
}

#[test]
fn collided_two_char_word_is_absent() {
    // 回归:「但是 dan shi」推导码 djui 命中规范全码(「蛋」),该 semantic entry
    // 已被提取期过滤;djui 不得作为词码出现。
    let (_, rows) = parse_dictionary(&generate_rime_word_dictionary());
    assert!(codes_of(&rows, "但是").is_empty(), "但是 不应出现在词词典");
    assert!(
        rows.iter().all(|(_, code, _)| code != "djui"),
        "djui 是规范全码,不得作为词码"
    );
}

#[test]
fn generation_is_byte_reproducible() {
    let first = generate_rime_word_dictionary();
    let second = generate_rime_word_dictionary();
    assert_eq!(first.as_bytes(), second.as_bytes());
}
