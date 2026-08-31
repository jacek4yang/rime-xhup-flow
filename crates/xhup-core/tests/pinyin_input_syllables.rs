//! `data/pinyin/xhup_input_syllables.txt` 规范数据的结构校验与哨兵检查。
//!
//! 清单文件是唯一的成员事实来源;本测试只验证格式、完整性与少量代表性条目,
//! 不在 Rust 代码中复制完整清单。

use std::fs;
use std::path::PathBuf;

use xhup_core::{DoublePinyinLayout, XhupInputSyllable};

fn data_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/pinyin/xhup_input_syllables.txt")
}

/// 读取清单并验证文件级结构:非空、纯 LF 行尾、以单个换行结尾。
fn read_inventory() -> Vec<String> {
    let path = data_path();
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("无法读取 {}: {err}", path.display()));
    assert!(!content.is_empty(), "清单文件不应为空");
    assert!(!content.contains('\r'), "清单不允许 CR 字符");
    assert!(content.ends_with('\n'), "清单应以单个换行结尾");

    let mut rows = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let row = index + 1;
        assert!(!line.is_empty(), "第 {row} 行为空行");
        assert_eq!(line.trim(), line, "第 {row} 行含首尾空白: {line:?}");
        assert!(
            line.bytes().all(|byte| byte.is_ascii_lowercase()),
            "第 {row} 行应为小写 ASCII 字母: {line:?}"
        );
        rows.push(line.to_string());
    }
    rows
}

#[test]
fn inventory_is_strictly_sorted_without_duplicates() {
    let rows = read_inventory();
    for pair in rows.windows(2) {
        assert!(
            pair[0] < pair[1],
            "清单未按字节序严格升序(重复或乱序):{:?} 在 {:?} 之前",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn inventory_has_exactly_406_rows() {
    assert_eq!(read_inventory().len(), 406, "规范清单应为 406 行");
}

#[test]
fn required_sentinels_are_present() {
    let rows = read_inventory();
    // 常规与边缘条目(含 ü 族的 v 表示),以及 j/q/x/y 后不改写的常规拼写
    for sentinel in [
        "a", "ang", "ba", "chua", "den", "dia", "kei", "lo", "me", "nou", "o", "pou", "shei", "yo",
        "lv", "lve", "nv", "nve", "ju", "qu", "xu", "yu", "jue", "que", "xue", "yue", "jun", "qun",
        "xun", "yun",
    ] {
        assert!(
            rows.iter().any(|row| row == sentinel),
            "缺少哨兵: {sentinel}"
        );
    }
}

#[test]
fn excluded_forms_are_absent() {
    let rows = read_inventory();
    // 项目编码边界排除(有上游来源支持,但当前小鹤布局无编码路径)
    for excluded in ["m", "n", "ng", "hng", "ea"] {
        assert!(
            !rows.iter().any(|row| row == excluded),
            "编码边界排除项不应出现: {excluded}"
        );
    }
    // 显式暂缓的扩展音节(仅出现于审计/交叉核对层)
    for deferred in ["biang", "cei", "fiao", "hm", "nun", "rua", "tei", "zhei"] {
        assert!(
            !rows.iter().any(|row| row == deferred),
            "暂缓扩展音节不应出现: {deferred}"
        );
    }
    // 结构可组合但非规范输入成员
    assert!(!rows.iter().any(|row| row == "biong"), "biong 不应出现");
}

#[test]
fn keyboard_composability_differs_from_inventory_membership() {
    // 架构不变量:键盘结构可组合 != XHUP 规范输入成员资格
    assert!(
        DoublePinyinLayout::canonical()
            .compose("b", "iong")
            .is_some()
    );
    assert!("biong".parse::<XhupInputSyllable>().is_err());
}

#[test]
fn every_inventory_row_is_accepted_by_domain_type() {
    for row in read_inventory() {
        let syllable: XhupInputSyllable = row
            .parse()
            .unwrap_or_else(|err| panic!("清单条目应通过领域校验: {row:?} ({err})"));
        assert_eq!(syllable.as_str(), row);
    }
}
