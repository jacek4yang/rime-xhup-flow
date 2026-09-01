//! 一级简码固定层:26 键显式方案映射的解析与校验。
//!
//! 入库 TSV `data/shortcuts/level1.tsv` 经 `include_str!` 嵌入,是一级简码的
//! 唯一事实来源(数据性质与 provenance 见该目录 README)。一级简码是显式方案
//! 设计数据,不由频率推导:每个字母键对应一个确定的规范汉字,是稳定的用户
//! 肌肉记忆兼容接口;修改映射属于 breaking scheme change。
//!
//! 声码一致性是构建时硬不变量:每个键必须等于对应汉字某个规范读音双拼码的
//! 首键(含 `啊 a` / `二 e` / `哦 o` 零声母与 `出 i` / `是 u` / `这 v`
//! 键位转换项)。解析时逐条断言,矛盾即 panic——不修改 layout 迎合映射。
//! 本模块不读写文件、不访问网络;TSV 损坏属于仓库不变量被破坏。

use std::collections::BTreeSet;
use std::sync::OnceLock;

use xhup_core::{Key, XhupHanzi};

/// 入库的一级简码 TSV(唯一事实来源)。
const LEVEL1_TSV: &str = include_str!("../../../data/shortcuts/level1.tsv");

/// canonical 序列化顺序:QWERTY 物理布局。
const QWERTY_ORDER: &str = "qwertyuiopasdfghjklzxcvbnm";

/// 一条一级简码关系:一个字母键对应一个确定的规范汉字。
///
/// 一级简码只提供一键精确候选(新增别名,不替换该字的 2/3/4 码关系),
/// 不包含权重、频率或编码推导信息。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Level1ShortcutEntry {
    key: Key,
    hanzi: XhupHanzi,
}

impl Level1ShortcutEntry {
    /// 该关系对应的按键(恰一个小写 ASCII 字母)。
    pub fn key(&self) -> Key {
        self.key
    }

    /// 该键对应的规范汉字。
    pub fn hanzi(&self) -> XhupHanzi {
        self.hanzi
    }
}

/// 全部 26 条一级简码关系(进程内共享,解析一次)。
///
/// 顺序为 canonical 序列化顺序:QWERTY 物理布局
/// (`qwertyuiop` / `asdfghjkl` / `zxcvbnm`)。
pub fn canonical_level1_shortcuts() -> &'static [Level1ShortcutEntry] {
    static ENTRIES: OnceLock<Vec<Level1ShortcutEntry>> = OnceLock::new();
    ENTRIES
        .get_or_init(|| parse_tsv(LEVEL1_TSV, "level1.tsv"))
        .as_slice()
}

/// 该汉字的某个规范读音的双拼码首键是否等于给定键(声码一致性判定)。
fn has_matching_sound_prefix(hanzi: XhupHanzi, key: Key) -> bool {
    hanzi
        .readings()
        .iter()
        .filter_map(|reading| reading.to_input_syllable())
        .any(|syllable| syllable.to_double_pinyin_code().as_slice()[0] == key)
}

/// 解析内嵌 TSV:`#` 开头为注释头;数据行 `键<TAB>汉字`。
///
/// 校验:恰好 26 行;键恰为一个小写 a-z 字母;汉字恰为一个规范汉字;
/// 键与汉字各自唯一;键序列恰为 QWERTY 物理顺序(蕴含键集合为完整 a-z);
/// 每条关系通过声码一致性硬断言。
fn parse_tsv(text: &'static str, name: &str) -> Vec<Level1ShortcutEntry> {
    let mut entries: Vec<Level1ShortcutEntry> = Vec::new();
    let mut keys: BTreeSet<Key> = BTreeSet::new();
    let mut hanzi_seen: BTreeSet<XhupHanzi> = BTreeSet::new();
    for (index, line) in text.lines().enumerate() {
        let row_number = index + 1;
        if line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('\t');
        let (Some(key_field), Some(hanzi_field), None) =
            (fields.next(), fields.next(), fields.next())
        else {
            panic!("{name} 第 {row_number} 行应为两个 TAB 分隔字段: {line:?}");
        };

        let mut key_chars = key_field.chars();
        let (Some(key_char), None) = (key_chars.next(), key_chars.next()) else {
            panic!("{name} 第 {row_number} 行键应恰为一个字符: {line:?}");
        };
        let key = Key::from_char(key_char)
            .unwrap_or_else(|_| panic!("{name} 第 {row_number} 行键应为小写 a-z: {line:?}"));

        let mut hanzi_chars = hanzi_field.chars();
        let (Some(hanzi_char), None) = (hanzi_chars.next(), hanzi_chars.next()) else {
            panic!("{name} 第 {row_number} 行汉字应恰为一个字: {line:?}");
        };
        let hanzi = XhupHanzi::try_from(hanzi_char)
            .unwrap_or_else(|_| panic!("{name} 第 {row_number} 行汉字不在规范清单内: {line:?}"));

        assert!(
            has_matching_sound_prefix(hanzi, key),
            "{name} 第 {row_number} 行声码不一致:键 {key} 不是 {hanzi} 任何规范读音双拼码的首键"
        );
        assert!(keys.insert(key), "{name} 第 {row_number} 行键重复: {key}");
        assert!(
            hanzi_seen.insert(hanzi),
            "{name} 第 {row_number} 行汉字重复: {hanzi}"
        );

        entries.push(Level1ShortcutEntry { key, hanzi });
    }
    assert_eq!(entries.len(), 26, "{name} 应恰好 26 行数据");
    let sequence: String = entries.iter().map(|entry| entry.key.as_char()).collect();
    assert_eq!(
        sequence, QWERTY_ORDER,
        "{name} 键序列应为 QWERTY 物理布局顺序"
    );
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn mapping_is_exactly_the_frozen_26() {
        let entries = canonical_level1_shortcuts();
        assert_eq!(entries.len(), 26);
        for (entry, &(key, hanzi)) in entries.iter().zip(FROZEN.iter()) {
            assert_eq!(entry.key(), Key::from_char(key).unwrap());
            assert_eq!(entry.hanzi().as_char(), hanzi);
        }
    }

    #[test]
    fn keys_are_qwerty_order_and_complete_alphabet() {
        let entries = canonical_level1_shortcuts();
        let sequence: String = entries.iter().map(|entry| entry.key().as_char()).collect();
        assert_eq!(sequence, QWERTY_ORDER, "canonical 序列为 QWERTY 物理布局");
        let sorted: String = {
            let mut keys: Vec<char> = sequence.chars().collect();
            keys.sort_unstable();
            keys.into_iter().collect()
        };
        assert_eq!(sorted, "abcdefghijklmnopqrstuvwxyz", "键集合为完整 a-z");
    }

    #[test]
    fn every_entry_passes_sound_prefix_audit() {
        for entry in canonical_level1_shortcuts() {
            assert!(
                has_matching_sound_prefix(entry.hanzi(), entry.key()),
                "{} 的某个规范读音双拼码首键应等于 {}",
                entry.hanzi(),
                entry.key()
            );
        }
    }
}
