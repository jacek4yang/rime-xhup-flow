//! 规范组合关系的穷举回归:`DoublePinyinCode + ShapeCode → FullCode`。
//!
//! 本测试只使用公开 canonical API 派生 FullCode,不读取任何 Rime 词典。
//! 锁定的计数是当前规范数据集的回归事实,不是 `FullCode` 的结构性语义;
//! 规范数据若有经审计的变更,这些计数可随之更新。
//!
//! 注意:集合比较仅用于测试的确定性呈现,`readings() × shape_codes()` 的
//! 嵌套迭代顺序不构成任何候选优先级/排序语义。

use std::collections::{BTreeMap, BTreeSet};

use xhup_core::{FullCode, XhupHanzi, XhupInputSyllable};

/// 某字的规范 FullCode 集合(测试用确定性集合;非候选顺序)。
fn fullcode_set(hanzi: char) -> BTreeSet<FullCode> {
    let hanzi = XhupHanzi::try_from(hanzi).unwrap();
    let mut set = BTreeSet::new();
    for &reading in hanzi.readings() {
        let Some(syllable) = reading.to_input_syllable() else {
            continue;
        };
        let sound = syllable.to_double_pinyin_code();
        for &shape in hanzi.shape_codes() {
            set.insert(FullCode::from_parts(sound, shape));
        }
    }
    set
}

struct CompositionStats {
    encodable_relations: usize,
    raw_tuples: usize,
    unique_relations: usize,
    fullcode_hanzis: BTreeMap<FullCode, BTreeSet<char>>,
    raw_hist: BTreeMap<usize, usize>,
    unique_hist: BTreeMap<usize, usize>,
    collapsed: Vec<(char, usize, usize)>,
    zero_fullcode: Vec<char>,
}

/// 单次遍历计算全部组合统计:读音关系、原始元组、唯一关系、碰撞、直方图、塌缩。
fn composition_stats() -> CompositionStats {
    let mut stats = CompositionStats {
        encodable_relations: 0,
        raw_tuples: 0,
        unique_relations: 0,
        fullcode_hanzis: BTreeMap::new(),
        raw_hist: BTreeMap::new(),
        unique_hist: BTreeMap::new(),
        collapsed: Vec::new(),
        zero_fullcode: Vec::new(),
    };
    for &hanzi in XhupHanzi::all() {
        let syllables: Vec<XhupInputSyllable> = hanzi
            .readings()
            .iter()
            .filter_map(|reading| reading.to_input_syllable())
            .collect();
        stats.encodable_relations += syllables.len();
        let shapes = hanzi.shape_codes();
        let raw = syllables.len() * shapes.len();
        stats.raw_tuples += raw;
        let fullcodes = fullcode_set(hanzi.as_char());
        if fullcodes.is_empty() {
            stats.zero_fullcode.push(hanzi.as_char());
        }
        if raw > fullcodes.len() {
            stats
                .collapsed
                .push((hanzi.as_char(), raw, fullcodes.len()));
        }
        stats.unique_relations += fullcodes.len();
        *stats.raw_hist.entry(raw).or_insert(0) += 1;
        *stats.unique_hist.entry(fullcodes.len()).or_insert(0) += 1;
        for fullcode in fullcodes {
            stats
                .fullcode_hanzis
                .entry(fullcode)
                .or_default()
                .insert(hanzi.as_char());
        }
    }
    stats
}

#[test]
fn every_canonical_composition_decomposes_back() {
    for &hanzi in XhupHanzi::all() {
        for &reading in hanzi.readings() {
            let Some(syllable) = reading.to_input_syllable() else {
                continue;
            };
            let sound = syllable.to_double_pinyin_code();
            for &shape in hanzi.shape_codes() {
                let full = FullCode::from_parts(sound, shape);
                assert_eq!(full.double_pinyin_code(), sound);
                assert_eq!(full.shape_code(), shape);
            }
        }
    }
}

#[test]
fn canonical_composition_counts_match_audit() {
    let stats = composition_stats();
    assert_eq!(stats.encodable_relations, 8574, "可编码规范读音关系");
    assert_eq!(stats.raw_tuples, 9159, "原始读音×形码元组");
    assert_eq!(stats.unique_relations, 9158, "唯一 (字, FullCode) 关系");
    assert_eq!(stats.fullcode_hanzis.len(), 8416, "全局不同 FullCode");
    let collided = stats
        .fullcode_hanzis
        .values()
        .filter(|hanzi| hanzi.len() > 1)
        .count();
    assert_eq!(collided, 658, "碰撞 FullCode 数");
    let max_share = stats
        .fullcode_hanzis
        .values()
        .map(BTreeSet::len)
        .max()
        .unwrap();
    assert_eq!(max_share, 5, "单码最大共享字数");
}

#[test]
fn collision_maximum_is_exactly_jumk_and_liml() {
    let stats = composition_stats();
    let max_codes: BTreeMap<FullCode, BTreeSet<char>> = stats
        .fullcode_hanzis
        .into_iter()
        .filter(|(_, hanzi)| hanzi.len() == 5)
        .collect();
    let expected: BTreeMap<FullCode, BTreeSet<char>> = [
        ("jumk", ['枸', '桔', '椐', '橘', '驹']),
        ("liml", ['朸', '枥', '栎', '粒', '骊']),
    ]
    .into_iter()
    .map(|(code, hanzi)| (code.parse().unwrap(), BTreeSet::from(hanzi)))
    .collect();
    assert_eq!(max_codes, expected);
}

#[test]
fn per_hanzi_histograms_match_audit() {
    let stats = composition_stats();
    let expected_raw: BTreeMap<usize, usize> = [(0, 2), (1, 7114), (2, 947), (3, 17), (4, 25)]
        .into_iter()
        .collect();
    let expected_unique: BTreeMap<usize, usize> = [(0, 2), (1, 7114), (2, 947), (3, 18), (4, 24)]
        .into_iter()
        .collect();
    assert_eq!(stats.raw_hist, expected_raw, "原始元组直方图");
    assert_eq!(stats.unique_hist, expected_unique, "唯一 FullCode 直方图");
}

#[test]
fn ge_collapse_is_the_lo_luo_code_collision() {
    let stats = composition_stats();
    // 唯一的字内塌缩:咯的 lo/luo 两个规范读音产生同一双拼码
    assert_eq!(stats.collapsed, [('咯', 4, 3)]);
    let lo: XhupInputSyllable = "lo".parse().unwrap();
    let luo: XhupInputSyllable = "luo".parse().unwrap();
    assert_eq!(lo.to_double_pinyin_code(), luo.to_double_pinyin_code());
    let readings: Vec<String> = XhupHanzi::try_from('咯')
        .unwrap()
        .readings()
        .iter()
        .map(|reading| reading.to_string())
        .collect();
    assert!(readings.contains(&"lo".to_string()) && readings.contains(&"luo".to_string()));
}

#[test]
fn zero_fullcode_boundary() {
    let stats = composition_stats();
    // 有形码 ≠ 有规范 FullCode:呣/嗯 无可编码规范读音,不得引入旧码 om 回退
    assert_eq!(stats.zero_fullcode, ['呣', '嗯']);
    for ch in ['呣', '嗯'] {
        let hanzi = XhupHanzi::try_from(ch).unwrap();
        assert!(!hanzi.shape_codes().is_empty(), "{ch} 仍有有效形码");
        assert_eq!(fullcode_set(ch), BTreeSet::new(), "{ch} 应无 FullCode 组合");
    }
}

#[test]
fn sentinel_fullcode_sets() {
    let cases: [(char, &[&str]); 8] = [
        ('啊', &["aakd", "aakk"]),
        ('阿', &["aaed", "aaek", "eeed", "eeek"]),
        ('贯', &["grgr", "grtr", "grvr"]),
        ('欻', &["ixhr", "xuhr"]),
        ('行', &["hgii", "hhii", "xkii"]),
        ('长', &["ihpn", "vhpn"]),
        ('呣', &[]),
        ('嗯', &[]),
    ];
    for (ch, expected) in cases {
        let expected: BTreeSet<FullCode> =
            expected.iter().map(|code| code.parse().unwrap()).collect();
        assert_eq!(fullcode_set(ch), expected, "{ch}");
    }
}
