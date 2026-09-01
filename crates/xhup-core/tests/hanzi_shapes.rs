//! `data/shape/hanzi_shapes.tsv` 规范数据的结构校验与领域 API 互证。
//!
//! 形码表是唯一的汉字形码事实来源;本测试只针对该文件本身验证格式、完整性与
//! 已审计的边界事实,不读取任何 Rime 词典,也不在 Rust 代码中复制完整数据。

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use xhup_core::{ShapeCode, XhupHanzi};

fn data_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/shape/hanzi_shapes.tsv")
}

struct Row {
    hanzi: char,
    code: String,
}

/// 读取形码表并验证行级结构:两字段、单字符、两键小写 ASCII 形码。
fn read_rows() -> Vec<Row> {
    let path = data_path();
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("无法读取 {}: {err}", path.display()));
    assert!(!content.is_empty(), "形码表不应为空");
    assert!(!content.contains('\r'), "形码表不允许 CR 字符");
    assert!(content.ends_with('\n'), "形码表应以单个换行结尾");
    assert!(!content.ends_with("\n\n"), "形码表应恰好以一个换行结尾");

    let mut rows = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let row = index + 1;
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(
            fields.len(),
            2,
            "第 {row} 行应为两个 TAB 分隔字段: {line:?}"
        );
        let mut chars = fields[0].chars();
        let hanzi = chars
            .next()
            .unwrap_or_else(|| panic!("第 {row} 行字符字段为空"));
        assert!(
            chars.next().is_none(),
            "第 {row} 行字符字段应恰好为一个字符: {line:?}"
        );
        let code = fields[1];
        assert!(
            code.len() == 2 && code.bytes().all(|byte| byte.is_ascii_lowercase()),
            "第 {row} 行形码应为两个小写 ASCII 字母: {line:?}"
        );
        rows.push(Row {
            hanzi,
            code: code.to_string(),
        });
    }
    rows
}

#[test]
fn canonical_counts_match_audit() {
    let rows = read_rows();
    assert_eq!(rows.len(), 8666, "规范形码表应为 8666 行");
    let chars: HashSet<char> = rows.iter().map(|row| row.hanzi).collect();
    assert_eq!(chars.len(), 8105, "规范汉字应恰好 8105 个");
    let distinct: HashSet<&str> = rows.iter().map(|row| row.code.as_str()).collect();
    assert_eq!(distinct.len(), 670, "不同形码应恰好 670 个");
}

#[test]
fn groups_are_contiguous_sorted_with_sorted_codes() {
    let rows = read_rows();
    let mut seen: HashSet<char> = HashSet::new();
    let mut cardinality = [0usize; 4]; // 每字形码数分布(1/2/3)
    let mut index = 0;
    while index < rows.len() {
        let hanzi = rows[index].hanzi;
        assert!(seen.insert(hanzi), "字符组不连续或重复: {hanzi:?}");
        if index > 0 {
            assert!(
                hanzi > rows[index - 1].hanzi,
                "字符组未按 Unicode 标量值严格升序: {:?} 之后出现 {hanzi:?}",
                rows[index - 1].hanzi
            );
        }
        let mut group_len = 1;
        while index + group_len < rows.len() && rows[index + group_len].hanzi == hanzi {
            let previous = &rows[index + group_len - 1].code;
            let current = &rows[index + group_len].code;
            assert!(
                previous < current,
                "组内形码未按字节序严格升序(重复或乱序): {hanzi:?} {previous} 之后出现 {current}"
            );
            group_len += 1;
        }
        assert!(group_len <= 3, "每字形码数超过审计最大值 3: {hanzi:?}");
        cardinality[group_len] += 1;
        index += group_len;
    }
    assert_eq!(cardinality[1], 7545, "1 形码字应为 7545 个");
    assert_eq!(cardinality[2], 559, "2 形码字应为 559 个");
    assert_eq!(cardinality[3], 1, "3 形码字应恰好 1 个");
    assert_eq!(cardinality[2] + cardinality[3], 560, "多形码字应为 560 个");
}

#[test]
fn sentinel_rows_are_present() {
    let rows = read_rows();
    let has = |hanzi: char, code: &str| {
        rows.iter()
            .any(|row| row.hanzi == hanzi && row.code == code)
    };
    for (hanzi, code) in [
        ('啊', "kd"),
        ('啊', "kk"),
        ('阿', "ed"),
        ('阿', "ek"),
        ('鞍', "gn"),
        ('鞍', "nn"),
        ('贯', "gr"),
        ('贯', "tr"),
        ('贯', "vr"),
        ('忒', "yx"),
        ('这', "zw"),
        ('欸', "sr"),
        ('呒', "kw"),
        ('呣', "km"),
    ] {
        assert!(has(hanzi, code), "缺少哨兵行: {hanzi} {code}");
    }
    // 贯是唯一 3 形码字
    let three: HashSet<char> = {
        let mut counts = std::collections::HashMap::new();
        for row in &rows {
            *counts.entry(row.hanzi).or_insert(0usize) += 1;
        }
        counts
            .into_iter()
            .filter(|(_, count)| *count == 3)
            .map(|(hanzi, _)| hanzi)
            .collect()
    };
    assert_eq!(three, HashSet::from(['贯']));
}

#[test]
fn raw_rows_match_domain_api() {
    // 原始数据与领域类型互证:每行的字在规范清单内,形码出现在该字规范形码集中
    let rows = read_rows();
    let mut api_total = 0usize;
    for row in &rows {
        let hanzi = XhupHanzi::try_from(row.hanzi)
            .unwrap_or_else(|err| panic!("形码表字符应在规范清单内: {:?} ({err})", row.hanzi));
        assert!(
            hanzi
                .shape_codes()
                .iter()
                .any(|code| code.to_string() == row.code),
            "{:?} 的规范形码应包含 {}",
            row.hanzi,
            row.code
        );
    }
    for &hanzi in XhupHanzi::all() {
        let codes = hanzi.shape_codes();
        assert!(!codes.is_empty(), "{:?} 缺少形码", hanzi.as_char());
        api_total += codes.len();
        for &code in codes {
            // 结构校验:恰好两键,Display 往返
            assert_eq!(code.as_slice().len(), 2);
            let reparsed: ShapeCode = code.to_string().parse().unwrap();
            assert_eq!(reparsed, code);
            assert!(
                rows.iter()
                    .any(|row| row.hanzi == hanzi.as_char() && row.code == code.to_string()),
                "领域形码 {} 应出现在原始表中({:?})",
                code,
                hanzi.as_char()
            );
        }
    }
    assert_eq!(api_total, 8666, "领域 API 形码关系总数应为 8666");
}
