//! 固定层静态单字编码条目:2/3/4 码推导、频率聚合、排名与最终化。
//!
//! 本模块是唯一的静态单字管线,Rime 词典与训练器 JSON 都是同一份最终化
//! 条目集的投影:
//!
//! ```text
//! 规范读音/规范形码(xhup-core)
//!     → 推导原始 2/3/4 码关系(2 码 = 双拼音码;3 码 = 音码 + 首形键;4 码 = 全码)
//!     → 按 (汉字, 码) 去重,贡献读音去重
//!     → 万象读音分数聚合(同一读音只计一次,多形路径不重复计分)
//!     → 按码分组排名(分数降序,Unicode 标量升序决胜)
//!     → 指派显式 Rime 权重(组内 N..1,正数且唯一)
//!     → 最终化条目集
//! ```
//!
//! 排名证据是万象聚合分数;Rime 权重只是排名结果的输出表示。条目的输出顺序
//! 是确定性的**序列化顺序**(码长升序、码字典序升序、权重降序、汉字 Unicode
//! 标量升序),不承担排名语义——同码候选顺序完全由显式权重表达。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use xhup_core::{HanziReading, KeySequence, XhupHanzi};

use crate::frequency::reading_score;

/// 一条最终化的静态单字编码关系(模块内投影的事实来源)。
///
/// 字段对 crate 内只读;公共 API 只暴露 [`RimeCharCodeEntry`] 投影,
/// 不暴露万象来源概念或内部排名结构。
pub(crate) struct FinalizedCharCodeEntry {
    hanzi: XhupHanzi,
    code: KeySequence,
    /// 贡献该 `(汉字, 码)` 关系的唯一规范读音(字典序升序)。
    readings: Box<[HanziReading]>,
    /// 贡献读音的万象聚合分数(u64,可为 0 = 无频率证据)。
    frequency_score: u64,
    /// 显式 Rime 权重(组内排名 N..1;正数、同码唯一、越大越靠前)。
    rime_weight: u32,
}

impl FinalizedCharCodeEntry {
    pub(crate) fn hanzi(&self) -> XhupHanzi {
        self.hanzi
    }

    pub(crate) fn code(&self) -> &KeySequence {
        &self.code
    }

    pub(crate) fn readings(&self) -> &[HanziReading] {
        &self.readings
    }

    pub(crate) fn frequency_score(&self) -> u64 {
        self.frequency_score
    }

    pub(crate) fn rime_weight(&self) -> u32 {
        self.rime_weight
    }
}

/// 最终化静态单字条目集(进程内共享,计算一次)。
///
/// 顺序为字典序列化顺序:码长升序 → 码字典序升序 → 权重降序 → 汉字升序。
pub(crate) fn finalized_char_code_entries() -> &'static [FinalizedCharCodeEntry] {
    static FINALIZED: OnceLock<Vec<FinalizedCharCodeEntry>> = OnceLock::new();
    FINALIZED.get_or_init(finalize).as_slice()
}

/// 公共投影:一条静态单字编码关系,携带显式 Rime 权重。
///
/// 表示固定层(2/3/4 码)中一个规范汉字的一个可接受静态编码。
/// 不携带万象来源概念;候选顺序由 `weight` 显式表达。
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RimeCharCodeEntry {
    hanzi: XhupHanzi,
    code: KeySequence,
    weight: u32,
}

impl RimeCharCodeEntry {
    /// 该条目对应的规范汉字。
    pub fn hanzi(&self) -> XhupHanzi {
        self.hanzi
    }

    /// 该条目对应的静态输入码(2/3/4 键)。
    pub fn code(&self) -> &KeySequence {
        &self.code
    }

    /// 显式 Rime 权重:同码候选中正数且唯一,越大排名越靠前。
    pub fn weight(&self) -> u32 {
        self.weight
    }
}

/// 全部静态单字编码条目(2/3/4 码)的公共投影。
///
/// 与 [`crate::generate_rime_char_dictionary`]、
/// [`crate::generate_trainer_dataset`] 共享同一份最终化条目集:
/// 三者是同一数据的不同视图,不存在第二份推导/排名实现。
///
/// 返回顺序:码长升序、码字典序升序、权重降序、汉字 Unicode 标量升序。
pub fn canonical_char_code_entries() -> Vec<RimeCharCodeEntry> {
    finalized_char_code_entries()
        .iter()
        .map(|entry| RimeCharCodeEntry {
            hanzi: entry.hanzi(),
            code: entry.code().clone(),
            weight: entry.rime_weight(),
        })
        .collect()
}

/// 推导原始关系并按 `(汉字, 码)` 归并贡献读音。
fn derive_contributions() -> BTreeMap<(XhupHanzi, KeySequence), BTreeSet<HanziReading>> {
    let mut contributions: BTreeMap<(XhupHanzi, KeySequence), BTreeSet<HanziReading>> =
        BTreeMap::new();
    for &hanzi in XhupHanzi::all() {
        for &reading in hanzi.readings() {
            let Some(syllable) = reading.to_input_syllable() else {
                continue;
            };
            let sound = syllable.to_double_pinyin_code();
            let s0 = sound.as_slice()[0];
            let s1 = sound.as_slice()[1];

            // 2 码:完整双拼音码(每个可编码规范读音)。
            let two = KeySequence::from_keys(&[s0, s1]).expect("两键非空");
            contributions
                .entry((hanzi, two))
                .or_default()
                .insert(reading);

            for &shape in hanzi.shape_codes() {
                let shape_keys = shape.as_slice();
                // 3 码:双拼音码 + 首形键。
                let three = KeySequence::from_keys(&[s0, s1, shape_keys[0]]).expect("三键非空");
                contributions
                    .entry((hanzi, three))
                    .or_default()
                    .insert(reading);
                // 4 码:规范全码(音码 + 形码)。
                let full = xhup_core::FullCode::from_parts(sound, shape);
                let four = KeySequence::from_keys(full.as_slice()).expect("四键非空");
                contributions
                    .entry((hanzi, four))
                    .or_default()
                    .insert(reading);
            }
        }
    }
    contributions
}

/// 聚合频率、组内排名、指派权重并按序列化顺序输出最终化条目集。
fn finalize() -> Vec<FinalizedCharCodeEntry> {
    let mut entries: Vec<FinalizedCharCodeEntry> = derive_contributions()
        .into_iter()
        .map(|((hanzi, code), readings)| {
            // 频率证据属于读音:同一读音只计一次,多形路径塌缩不重复计分。
            let frequency_score = readings.iter().fold(0u64, |sum, &reading| {
                sum.checked_add(reading_score(hanzi, reading))
                    .expect("聚合分数 u64 溢出")
            });
            FinalizedCharCodeEntry {
                hanzi,
                code,
                readings: readings.into_iter().collect(),
                frequency_score,
                rime_weight: 0, // 排名后回填
            }
        })
        .collect();

    // 按码分组排名:聚合分数降序,汉字 Unicode 标量升序为最终决胜。
    entries.sort_by(|a, b| {
        a.code
            .cmp(&b.code)
            .then(b.frequency_score.cmp(&a.frequency_score))
            .then(a.hanzi.cmp(&b.hanzi))
    });
    let mut group_start = 0;
    while group_start < entries.len() {
        let mut group_end = group_start + 1;
        while group_end < entries.len() && entries[group_end].code == entries[group_start].code {
            group_end += 1;
        }
        let group_size = group_end - group_start;
        for (rank, entry) in entries[group_start..group_end].iter_mut().enumerate() {
            // 第 1 名权重 N,末名权重 1:正数、同码唯一、越大越靠前。
            entry.rime_weight = u32::try_from(group_size - rank).expect("同码候选数超出 u32");
        }
        group_start = group_end;
    }

    // 序列化顺序:码长升序 → 码字典序升序 → 权重降序 → 汉字升序。
    entries.sort_by(|a, b| {
        a.code
            .len()
            .cmp(&b.code.len())
            .then(a.code.cmp(&b.code))
            .then(b.rime_weight.cmp(&a.rime_weight))
            .then(a.hanzi.cmp(&b.hanzi))
    });
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes_of(ch: char) -> BTreeSet<String> {
        finalized_char_code_entries()
            .iter()
            .filter(|entry| entry.hanzi().as_char() == ch)
            .map(|entry| entry.code().to_string())
            .collect()
    }

    #[test]
    fn relation_counts_match_audit() {
        let entries = finalized_char_code_entries();
        assert_eq!(entries.len(), 26753);
        for (len, expected) in [(2, 8573), (3, 9022), (4, 9158)] {
            assert_eq!(
                entries.iter().filter(|e| e.code().len() == len).count(),
                expected,
                "{len} 码关系数"
            );
        }
    }

    #[test]
    fn distinct_code_counts_match_audit() {
        let entries = finalized_char_code_entries();
        for (len, expected) in [(2, 405), (3, 4812), (4, 8416)] {
            let codes: BTreeSet<&KeySequence> = entries
                .iter()
                .filter(|e| e.code().len() == len)
                .map(FinalizedCharCodeEntry::code)
                .collect();
            assert_eq!(codes.len(), expected, "{len} 码 distinct 数");
        }
    }

    #[test]
    fn fanout_sentinels_match_audit() {
        let entries = finalized_char_code_entries();
        for (code, expected) in [("yi", 136), ("jid", 14), ("jumk", 5)] {
            assert_eq!(
                entries
                    .iter()
                    .filter(|e| e.code().to_string() == code)
                    .count(),
                expected,
                "{code} 扇出"
            );
        }
    }

    #[test]
    fn domain_sentinel_code_sets() {
        // 行:三读音 × 单形
        assert_eq!(
            codes_of('行'),
            BTreeSet::from([
                "hg".to_string(),
                "hh".to_string(),
                "xk".to_string(),
                "hgi".to_string(),
                "hhi".to_string(),
                "xki".to_string(),
                "hgii".to_string(),
                "hhii".to_string(),
                "xkii".to_string(),
            ])
        );
        // 长:两读音
        assert!(codes_of('长').contains("ihp") && codes_of('长').contains("vhp"));
        // 贯:三个形码首形键不同,3 码不塌缩(grg/grt/grv)
        assert_eq!(
            codes_of('贯'),
            BTreeSet::from([
                "gr".to_string(),
                "grg".to_string(),
                "grgr".to_string(),
                "grt".to_string(),
                "grtr".to_string(),
                "grv".to_string(),
                "grvr".to_string(),
            ])
        );
        // 咯:lo/luo 通用塌缩(四读音 × 单形码 kk → 3+3+3)
        assert_eq!(
            codes_of('咯'),
            BTreeSet::from([
                "ge".to_string(),
                "ka".to_string(),
                "lo".to_string(),
                "gek".to_string(),
                "kak".to_string(),
                "lok".to_string(),
                "gekk".to_string(),
                "kakk".to_string(),
                "lokk".to_string(),
            ])
        );
        // 呣/嗯:无可编码读音,零条目
        assert!(codes_of('呣').is_empty());
        assert!(codes_of('嗯').is_empty());
    }

    #[test]
    fn weights_are_positive_and_unique_within_code() {
        let entries = finalized_char_code_entries();
        let mut by_code: BTreeMap<&KeySequence, Vec<u32>> = BTreeMap::new();
        for entry in entries {
            assert!(entry.rime_weight() > 0, "权重必须为正");
            by_code
                .entry(entry.code())
                .or_default()
                .push(entry.rime_weight());
        }
        for (code, weights) in &by_code {
            let unique: BTreeSet<u32> = weights.iter().copied().collect();
            assert_eq!(unique.len(), weights.len(), "{code} 同码权重应唯一");
            assert_eq!(*unique.iter().next().unwrap(), 1, "{code} 最小权重为 1");
            assert_eq!(
                *unique.iter().next_back().unwrap(),
                weights.len() as u32,
                "{code} 最大权重为组大小"
            );
        }
    }

    #[test]
    fn contributing_readings_are_unique_and_sorted() {
        for entry in finalized_char_code_entries() {
            let readings = entry.readings();
            for pair in readings.windows(2) {
                assert!(pair[0] < pair[1], "贡献读音唯一且字典序升序");
            }
        }
        // 咯 lok:lo 与 luo 都真实贡献
        let ge = finalized_char_code_entries()
            .iter()
            .find(|entry| entry.hanzi().as_char() == '咯' && entry.code().to_string() == "lok")
            .expect("咯 lok 应存在");
        let readings: Vec<&str> = ge.readings().iter().map(|r| r.as_str()).collect();
        assert_eq!(readings, ["lo", "luo"]);
    }

    #[test]
    fn yi_group_ranking_orders_by_frequency() {
        let entries = finalized_char_code_entries();
        let mut group: Vec<&FinalizedCharCodeEntry> = entries
            .iter()
            .filter(|e| e.code().to_string() == "yi")
            .collect();
        group.sort_by_key(|a| std::cmp::Reverse(a.rime_weight()));
        // 组首应是万象分数最高的候选,且权重 = 组大小(万象:以 > 一)
        assert_eq!(group[0].rime_weight() as usize, group.len());
        assert_eq!(group[0].hanzi().as_char(), '以');
        assert_eq!(group[1].hanzi().as_char(), '一');
        // 同组内权重降序 ⟺ 分数降序(tie 时 Unicode 升序)
        for pair in group.windows(2) {
            assert!(pair[0].frequency_score() >= pair[1].frequency_score());
            if pair[0].frequency_score() == pair[1].frequency_score() {
                assert!(pair[0].hanzi() < pair[1].hanzi());
            }
        }
    }

    #[test]
    fn serialization_order_is_total_and_deterministic() {
        let entries = finalized_char_code_entries();
        for pair in entries.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            // 同码内权重唯一,故 (码长, 码, 权重降序) 已构成严格全序
            assert!(
                (a.code().len(), a.code(), u32::MAX - a.rime_weight())
                    < (b.code().len(), b.code(), u32::MAX - b.rime_weight()),
                "序列化顺序应严格递增"
            );
        }
    }

    #[test]
    fn score_zero_candidates_still_get_positive_weight() {
        // 呒 wu 无万象频率证据,但其编码条目仍存在且权重为正
        let entries: Vec<&FinalizedCharCodeEntry> = finalized_char_code_entries()
            .iter()
            .filter(|e| e.hanzi().as_char() == '呒')
            .collect();
        assert!(!entries.is_empty());
        assert!(entries.iter().all(|e| e.rime_weight() > 0));
    }
}
