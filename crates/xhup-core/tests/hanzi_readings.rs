//! `data/hanzi/readings.tsv` 规范数据的结构校验、边界审计回归与全链路编码检查。
//!
//! 读音表是唯一的汉字成员与读音事实来源;本测试只验证格式、完整性、已审计的边界
//! 事实与全量链路不变量,不在 Rust 代码中复制完整数据。

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use xhup_core::{XhupHanzi, XhupInputSyllable};

fn data_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/hanzi/readings.tsv")
}

struct Row {
    hanzi: char,
    reading: String,
    is_primary: bool,
}

/// 读取读音表并验证行级结构:三字段、单字符、小写 ASCII 读音、合法角色。
fn read_rows() -> Vec<Row> {
    let path = data_path();
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("无法读取 {}: {err}", path.display()));
    assert!(!content.is_empty(), "读音表不应为空");
    assert!(!content.contains('\r'), "读音表不允许 CR 字符");
    assert!(content.ends_with('\n'), "读音表应以单个换行结尾");
    assert!(!content.ends_with("\n\n"), "读音表应恰好以一个换行结尾");

    let mut rows = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let row = index + 1;
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(
            fields.len(),
            3,
            "第 {row} 行应为三个 TAB 分隔字段: {line:?}"
        );
        let mut chars = fields[0].chars();
        let hanzi = chars
            .next()
            .unwrap_or_else(|| panic!("第 {row} 行字符字段为空"));
        assert!(
            chars.next().is_none(),
            "第 {row} 行字符字段应恰好为一个字符: {line:?}"
        );
        let reading = fields[1];
        assert!(
            !reading.is_empty() && reading.bytes().all(|byte| byte.is_ascii_lowercase()),
            "第 {row} 行读音应为非空小写 ASCII 字母: {line:?}"
        );
        let is_primary = match fields[2] {
            "primary" => true,
            "alt" => false,
            role => panic!("第 {row} 行角色应为 primary 或 alt: {role:?}"),
        };
        rows.push(Row {
            hanzi,
            reading: reading.to_string(),
            is_primary,
        });
    }
    rows
}

#[test]
fn row_and_group_counts_match_approved_totals() {
    let rows = read_rows();
    assert_eq!(rows.len(), 8580, "规范读音表应为 8580 行");
    let chars: HashSet<char> = rows.iter().map(|row| row.hanzi).collect();
    assert_eq!(chars.len(), 8105, "规范汉字应恰好 8105 个");
    let primaries = rows.iter().filter(|row| row.is_primary).count();
    assert_eq!(primaries, 8105, "primary 行应恰好 8105 个");
    assert_eq!(rows.len() - primaries, 475, "alt 行应恰好 475 个");
}

#[test]
fn groups_are_contiguous_sorted_with_primary_first() {
    let rows = read_rows();
    let mut seen: HashSet<char> = HashSet::new();
    let mut index = 0;
    while index < rows.len() {
        let hanzi = rows[index].hanzi;
        assert!(seen.insert(hanzi), "字符组不连续或重复: {hanzi:?}");
        if let Some(previous) = index.checked_sub(1).map(|i| rows[i].hanzi) {
            assert!(
                hanzi > previous,
                "字符组未按 Unicode 标量值严格升序: {previous:?} 之后出现 {hanzi:?}"
            );
        }
        assert!(rows[index].is_primary, "每组首行应为 primary: {hanzi:?}");
        let mut group_len = 1;
        let mut last_alt: Option<&str> = None;
        while index + group_len < rows.len() && rows[index + group_len].hanzi == hanzi {
            let row = &rows[index + group_len];
            assert!(!row.is_primary, "每组恰有一个 primary: {hanzi:?}");
            if let Some(last) = last_alt {
                assert!(
                    last < row.reading.as_str(),
                    "alt 读音未按字节序严格升序(重复或乱序): {hanzi:?} {last} 之后出现 {}",
                    row.reading
                );
            }
            assert_ne!(
                row.reading, rows[index].reading,
                "alt 读音不应与 primary 重复: {hanzi:?}"
            );
            last_alt = Some(&row.reading);
            group_len += 1;
        }
        index += group_len;
    }
}

#[test]
fn sentinel_rows_are_present() {
    let rows = read_rows();
    let has = |hanzi: char, reading: &str, is_primary: bool| {
        rows.iter()
            .any(|row| row.hanzi == hanzi && row.reading == reading && row.is_primary == is_primary)
    };
    for (hanzi, reading, is_primary) in [
        ('一', "yi", true),
        ('行', "xing", true),
        ('行', "hang", false),
        ('行', "heng", false),
        ('欻', "chua", true),
        ('欻', "xu", false),
        ('呣', "m", true),
        ('嗯', "n", true),
        ('嗯', "ng", false),
        ('欸', "ea", false),
        ('剋', "kei", false),
        ('嗲', "dia", false),
    ] {
        assert!(
            has(hanzi, reading, is_primary),
            "缺少哨兵行: {hanzi} {reading}"
        );
    }
}

#[test]
fn non_xhup_boundary_matches_audited_set() {
    // 与再生成机械验证一致:恰好 6 个非 XHUP 关系、5 个受影响字、2 个非 XHUP 主读音
    let rows = read_rows();
    let boundary: Vec<(char, &str, bool)> = rows
        .iter()
        .filter(|row| row.reading.parse::<XhupInputSyllable>().is_err())
        .map(|row| (row.hanzi, row.reading.as_str(), row.is_primary))
        .collect();
    assert_eq!(
        boundary,
        [
            ('呒', "m", false),
            ('呣', "m", true),
            ('哼', "hng", false),
            ('嗯', "n", true),
            ('嗯', "ng", false),
            ('欸', "ea", false),
        ]
    );
    let affected: HashSet<char> = boundary.iter().map(|row| row.0).collect();
    assert_eq!(affected.len(), 5);
    assert_eq!(boundary.iter().filter(|row| row.2).count(), 2);
}

#[test]
fn exhaustive_canonical_chain_encodes() {
    // 全链路:规范汉字 → 来源读音 →(可转换时)规范输入音节 → 小鹤双拼码。
    // 不断言每个读音可转换,也不断言每字有可编码读音。
    let mut encodable = 0usize;
    for &hanzi in XhupHanzi::all() {
        let readings = hanzi.readings();
        assert!(!readings.is_empty());
        assert_eq!(hanzi.primary_reading(), readings[0]);
        for &reading in readings {
            if let Some(syllable) = reading.to_input_syllable() {
                let _code = syllable.to_double_pinyin_code();
                encodable += 1;
            }
        }
    }
    // 审计事实:8580 个读音关系中 8574 个可编码(8580 - 6 个非 XHUP 边界)
    assert_eq!(encodable, 8574);
}

#[test]
fn raw_rows_match_domain_api() {
    // 原始数据与领域类型互证:每行的字在清单内,且读音出现在该字的规范读音中
    let rows = read_rows();
    for row in &rows {
        let hanzi = XhupHanzi::try_from(row.hanzi)
            .unwrap_or_else(|err| panic!("读音表字符应在规范清单内: {:?} ({err})", row.hanzi));
        assert!(
            hanzi.readings().iter().any(|r| r.as_str() == row.reading),
            "{:?} 的规范读音应包含 {}",
            row.hanzi,
            row.reading
        );
        assert_eq!(
            row.is_primary,
            hanzi.primary_reading().as_str() == row.reading
        );
    }
}
