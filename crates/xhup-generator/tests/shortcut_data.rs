//! 一级简码规范数据(显式方案映射)的数据不变量测试(离线)。
//!
//! 锁定 26 键 frozen mapping、QWERTY canonical 序列化顺序、TSV 结构不变量与
//! 声码一致性;不访问网络,不测试任何上游当前状态。

use std::collections::BTreeSet;
use std::path::Path;

use xhup_core::{Key, XhupHanzi};
use xhup_generator::canonical_level1_shortcuts;

const SHORTCUTS_DIR: &str = "../../data/shortcuts";

fn tsv() -> String {
    std::fs::read_to_string(Path::new(SHORTCUTS_DIR).join("level1.tsv"))
        .expect("应能读取入库一级简码 TSV")
}

/// 数据行 `(键, 汉字)`。
fn data_rows(text: &str) -> Vec<(char, char)> {
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let mut fields = line.split('\t');
            let (Some(key), Some(hanzi), None) = (fields.next(), fields.next(), fields.next())
            else {
                panic!("一级简码数据行应恰好为两列 `键<TAB>汉字`: {line:?}");
            };
            let mut key_chars = key.chars();
            let (Some(key), None) = (key_chars.next(), key_chars.next()) else {
                panic!("键应恰为一个字符: {line:?}");
            };
            let mut hanzi_chars = hanzi.chars();
            let (Some(hanzi), None) = (hanzi_chars.next(), hanzi_chars.next()) else {
                panic!("汉字应恰为一个字: {line:?}");
            };
            (key, hanzi)
        })
        .collect()
}

/// 冻结的 26 键一级简码映射(QWERTY 物理布局顺序)。任何改动都必须失败。
const FROZEN: [(char, char); 26] = [
    ('q', '去'),
    ('w', '我'),
    ('e', '二'),
    ('r', '人'),
    ('t', '他'),
    ('y', '一'),
    ('u', '是'),
    ('i', '出'),
    ('o', '哦'),
    ('p', '平'),
    ('a', '啊'),
    ('s', '三'),
    ('d', '的'),
    ('f', '非'),
    ('g', '个'),
    ('h', '和'),
    ('j', '就'),
    ('k', '可'),
    ('l', '了'),
    ('z', '在'),
    ('x', '小'),
    ('c', '才'),
    ('v', '这'),
    ('b', '不'),
    ('n', '你'),
    ('m', '没'),
];

#[test]
fn tsv_is_utf8_lf_only_with_single_final_newline() {
    let text = tsv();
    assert!(!text.starts_with('\u{feff}'), "无 BOM");
    assert!(!text.contains('\r'), "LF only");
    assert!(
        text.ends_with('\n') && !text.ends_with("\n\n"),
        "恰好一个末尾换行"
    );
    assert!(text.contains("# rows: 26"), "注释头声明行数");
}

#[test]
fn tsv_rows_exactly_match_frozen_mapping_in_qwerty_order() {
    let text = tsv();
    let rows = data_rows(&text);
    assert_eq!(rows.len(), 26, "恰好 26 行");
    assert_eq!(rows, FROZEN, "完整 26 键映射与 QWERTY 顺序被冻结锁定");
}

#[test]
fn keys_and_hanzi_are_unique_and_complete() {
    let text = tsv();
    let rows = data_rows(&text);
    let keys: BTreeSet<char> = rows.iter().map(|(key, _)| *key).collect();
    let hanzi: BTreeSet<char> = rows.iter().map(|(_, zi)| *zi).collect();
    assert_eq!(keys.len(), 26, "键唯一");
    assert_eq!(hanzi.len(), 26, "汉字唯一");
    let alphabet: String = keys.iter().collect();
    assert_eq!(alphabet, "abcdefghijklmnopqrstuvwxyz", "键集合为完整 a-z");
    for (key, _) in &rows {
        assert!(key.is_ascii_lowercase(), "键为小写 a-z: {key}");
    }
}

#[test]
fn every_hanzi_is_canonical() {
    let text = tsv();
    for (_, zi) in data_rows(&text) {
        XhupHanzi::try_from(zi).unwrap_or_else(|_| panic!("{zi} 应在规范清单内"));
    }
}

#[test]
fn every_shortcut_matches_sound_prefix_via_canonical_core() {
    // 声码一致性(经公共 API 独立验证):shortcut key == 该字某个规范读音
    // 双拼码的首键;零声母(啊 a / 二 e / 哦 o)与键位转换(出 i / 是 u /
    // 这 v)项同样必须成立。
    let entries = canonical_level1_shortcuts();
    assert_eq!(entries.len(), 26);
    for entry in entries {
        let hanzi = entry.hanzi();
        let key = entry.key();
        let matched = hanzi
            .readings()
            .iter()
            .filter_map(|reading| reading.to_input_syllable())
            .any(|syllable| syllable.to_double_pinyin_code().as_slice()[0] == key);
        assert!(matched, "{} 的某个规范读音双拼码首键应等于 {}", hanzi, key);
    }
    // 显式锁定零声母与键位转换代表项的推导路径
    let first_key_of = |zi: char, spelling: &str| -> Key {
        let hanzi = XhupHanzi::try_from(zi).unwrap();
        let reading = hanzi
            .readings()
            .iter()
            .copied()
            .find(|r| r.as_str() == spelling)
            .unwrap_or_else(|| panic!("{zi} 应有规范读音 {spelling}"));
        reading
            .to_input_syllable()
            .unwrap()
            .to_double_pinyin_code()
            .as_slice()[0]
    };
    assert_eq!(first_key_of('啊', "a").as_char(), 'a');
    assert_eq!(first_key_of('二', "er").as_char(), 'e');
    assert_eq!(first_key_of('哦', "o").as_char(), 'o');
    assert_eq!(first_key_of('出', "chu").as_char(), 'i');
    assert_eq!(first_key_of('是', "shi").as_char(), 'u');
    assert_eq!(first_key_of('这', "zhe").as_char(), 'v');
}

#[test]
fn readme_documents_design_nature_and_compatibility_policy() {
    let readme = std::fs::read_to_string(Path::new(SHORTCUTS_DIR).join("README.md")).unwrap();
    for needle in [
        "显式设计数据",
        "不是由万象词频",
        "https://www.flypy.cc/win_record.html",
        "冻结并交叉核对",
        "breaking",
        "不自动上屏",
    ] {
        assert!(readme.contains(needle), "README 缺少 `{needle}`");
    }
    // 不虚构官方页面直接列出完整 26 字映射
    assert!(
        !readme.contains("官方页面直接列出"),
        "不得声称官方页面直接列出完整映射"
    );
}
