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
//!     → 指派显式 Rime 权重(组内 N..1;与词层碰撞的 4 键码改用
//!       [`crate::merged_ranking`] 的跨表合并权重,正数且唯一)
//!     → 最终化条目集
//! ```
//!
//! 排名证据是万象聚合分数;Rime 权重只是排名结果的输出表示。条目的输出顺序
//! 是确定性的**序列化顺序**(码长升序、码字典序升序、权重降序、汉字 Unicode
//! 标量升序),不承担排名语义——同码候选顺序完全由显式权重表达。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use xhup_core::{HanziReading, InputHanzi, KeySequence, XhupHanzi};

use crate::frequency::reading_score;

/// 一条最终化的静态单字编码关系(模块内投影的事实来源)。
///
/// 字段对 crate 内只读;公共 API 只暴露 [`RimeCharCodeEntry`] 投影,
/// 不暴露万象来源概念或内部排名结构。
pub(crate) struct FinalizedCharCodeEntry {
    hanzi: InputHanzi,
    code: KeySequence,
    /// 贡献该 `(汉字, 码)` 关系的唯一规范读音(字典序升序)。
    readings: Box<[HanziReading]>,
    /// 贡献读音的万象聚合分数(u64,可为 0 = 无频率证据)。
    frequency_score: u64,
    /// `true` 表示关系来自原有规范读音 × 规范形码推导；扩展关系始终排在其后。
    core_derived: bool,
    /// 是否至少有一份小鹤官网 oracle 证据。
    official: bool,
    /// 贡献来源标识，字典序升序。
    sources: Box<[&'static str]>,
    /// 贡献来源状态，字典序升序。
    statuses: Box<[&'static str]>,
    /// 显式 Rime 权重(组内排名 N..1;正数、同码唯一、越大越靠前)。
    rime_weight: u32,
}

impl FinalizedCharCodeEntry {
    pub(crate) fn hanzi(&self) -> InputHanzi {
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

    pub(crate) fn is_core_derived(&self) -> bool {
        self.core_derived
    }

    pub(crate) fn is_official(&self) -> bool {
        self.official
    }

    pub(crate) fn sources(&self) -> &[&'static str] {
        &self.sources
    }

    pub(crate) fn statuses(&self) -> &[&'static str] {
        &self.statuses
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

/// 生产输入字符编码投影：包含规范 core 与有来源的扩展关系。
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RimeInputCharCodeEntry {
    hanzi: InputHanzi,
    code: KeySequence,
    weight: u32,
    frequency_score: u64,
    core_derived: bool,
    official: bool,
    sources: Box<[&'static str]>,
    statuses: Box<[&'static str]>,
}

impl RimeInputCharCodeEntry {
    pub fn hanzi(&self) -> InputHanzi {
        self.hanzi
    }

    pub fn code(&self) -> &KeySequence {
        &self.code
    }

    pub fn weight(&self) -> u32 {
        self.weight
    }

    pub fn frequency_score(&self) -> u64 {
        self.frequency_score
    }

    pub fn is_core_derived(&self) -> bool {
        self.core_derived
    }

    pub fn is_official(&self) -> bool {
        self.official
    }

    pub fn sources(&self) -> &[&'static str] {
        &self.sources
    }

    pub fn statuses(&self) -> &[&'static str] {
        &self.statuses
    }
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
        .filter(|entry| entry.is_core_derived())
        .map(|entry| RimeCharCodeEntry {
            hanzi: entry
                .hanzi()
                .core_standard()
                .expect("core-derived 关系必然属于规范核心"),
            code: entry.code().clone(),
            weight: entry.rime_weight(),
        })
        .collect()
}

/// 全部生产输入字符编码关系（规范 core + attested 扩展）。
pub fn canonical_input_char_code_entries() -> Vec<RimeInputCharCodeEntry> {
    finalized_char_code_entries()
        .iter()
        .map(|entry| RimeInputCharCodeEntry {
            hanzi: entry.hanzi(),
            code: entry.code().clone(),
            weight: entry.rime_weight(),
            frequency_score: entry.frequency_score(),
            core_derived: entry.is_core_derived(),
            official: entry.is_official(),
            sources: entry.sources().into(),
            statuses: entry.statuses().into(),
        })
        .collect()
}

#[derive(Default)]
struct Contribution {
    readings: BTreeSet<HanziReading>,
    source_weight: u32,
    official: bool,
    sources: BTreeSet<&'static str>,
    statuses: BTreeSet<&'static str>,
}

/// 推导原始关系并按 `(汉字, 码)` 归并贡献读音。
fn derive_core_contributions() -> BTreeMap<(InputHanzi, KeySequence), Contribution> {
    let mut contributions: BTreeMap<(InputHanzi, KeySequence), Contribution> = BTreeMap::new();
    for &hanzi in XhupHanzi::all() {
        let input = InputHanzi::try_from(hanzi.as_char())
            .expect("attested 输入字符层必须覆盖全部规范核心字");
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
                .entry((input, two))
                .or_default()
                .readings
                .insert(reading);

            for &shape in hanzi.shape_codes() {
                let shape_keys = shape.as_slice();
                // 3 码:双拼音码 + 首形键。
                let three = KeySequence::from_keys(&[s0, s1, shape_keys[0]]).expect("三键非空");
                contributions
                    .entry((input, three))
                    .or_default()
                    .readings
                    .insert(reading);
                // 4 码:规范全码(音码 + 形码)。
                let full = xhup_core::FullCode::from_parts(sound, shape);
                let four = KeySequence::from_keys(full.as_slice()).expect("四键非空");
                contributions
                    .entry((input, four))
                    .or_default()
                    .readings
                    .insert(reading);
            }
        }
    }
    contributions
}

/// 在旧 core 关系之上合入有来源编码。关系合并不做读音反推；每份 evidence
/// 只产生自身的 2/3/4 键路径，不对音码和形码做笛卡尔积。
fn derive_contributions() -> BTreeMap<(InputHanzi, KeySequence), Contribution> {
    let mut contributions = derive_core_contributions();
    for evidence in xhup_core::AttestedXhupCode::all() {
        let sound = evidence.sound_code();
        let shape = evidence.shape_code();
        let sound_keys = sound.as_slice();
        let shape_keys = shape.as_slice();
        let codes = [
            KeySequence::from_keys(sound_keys).expect("事实音码恰好两键"),
            KeySequence::from_keys(&[sound_keys[0], sound_keys[1], shape_keys[0]])
                .expect("事实三码恰好三键"),
            KeySequence::from_keys(evidence.full_code().as_slice()).expect("事实全码恰好四键"),
        ];
        for code in codes {
            let contribution = contributions.entry((evidence.hanzi(), code)).or_default();
            contribution.source_weight = contribution.source_weight.max(evidence.source_weight());
            contribution.official |= evidence.is_official();
            contribution.sources.insert(evidence.source());
            contribution.statuses.insert(evidence.status());
        }
    }
    contributions
}

/// 聚合频率、组内排名、指派权重并按序列化顺序输出最终化条目集。
fn finalize() -> Vec<FinalizedCharCodeEntry> {
    let mut entries = scored_entries();

    // 按码分组排名:聚合分数降序,汉字 Unicode 标量升序为最终决胜。
    entries.sort_by(|a, b| {
        a.code
            .cmp(&b.code)
            .then(b.core_derived.cmp(&a.core_derived))
            .then(b.official.cmp(&a.official))
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

    // 与 v2 简码或词层碰撞的码:改用跨表合并权重(保证全组无平局)。
    for entry in &mut entries {
        if let Some(weight) =
            crate::merged_ranking::merged_weight(&entry.code, &entry.hanzi.as_char().to_string())
        {
            entry.rime_weight = weight;
        }
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

/// 聚合频率后的未加权条目(rime_weight 占位 0)。
fn score_contributions(
    contributions: BTreeMap<(InputHanzi, KeySequence), Contribution>,
) -> Vec<FinalizedCharCodeEntry> {
    contributions
        .into_iter()
        .map(|((hanzi, code), contribution)| {
            // 频率证据属于读音:同一读音只计一次,多形路径塌缩不重复计分。
            let core_derived = !contribution.readings.is_empty();
            let frequency_score = if core_derived {
                let core = hanzi
                    .core_standard()
                    .expect("有规范读音贡献的关系必然属于 core");
                contribution.readings.iter().fold(0u64, |sum, &reading| {
                    sum.checked_add(reading_score(core, reading))
                        .expect("聚合分数 u64 溢出")
                })
            } else {
                u64::from(contribution.source_weight)
            };
            FinalizedCharCodeEntry {
                hanzi,
                code,
                readings: contribution.readings.into_iter().collect(),
                frequency_score,
                core_derived,
                official: contribution.official,
                sources: contribution.sources.into_iter().collect(),
                statuses: contribution.statuses.into_iter().collect(),
                rime_weight: 0, // 排名后回填
            }
        })
        .collect()
}

fn scored_entries() -> Vec<FinalizedCharCodeEntry> {
    score_contributions(derive_contributions())
}

/// 全部未加权条目快照,供 merged_ranking 重建 baseline 组。
/// 不触发最终化/权重逻辑,因此不会形成 OnceLock 初始化环。
pub(crate) fn scored_all_entries() -> Vec<crate::merged_ranking::ScoredEntry> {
    score_contributions(derive_core_contributions())
        .into_iter()
        .map(|entry| crate::merged_ranking::ScoredEntry {
            code: entry.code,
            text: entry.hanzi.as_char().to_string(),
            score: entry.frequency_score,
        })
        .collect()
}

/// 仅 attested 新增关系的未加权快照；merged ranking 将其追加到旧菜单之后。
pub(crate) fn scored_extension_entries() -> Vec<crate::merged_ranking::ScoredEntry> {
    scored_entries()
        .into_iter()
        .filter(|entry| !entry.core_derived)
        .map(|entry| crate::merged_ranking::ScoredEntry {
            code: entry.code,
            text: entry.hanzi.as_char().to_string(),
            score: entry.frequency_score,
        })
        .collect()
}

/// 未加权的 4 键条目快照,供 [`crate::merged_ranking`] 做跨表碰撞仲裁。
///
/// 只读取规范数据与频率表,不触发任何最终化/权重逻辑,因此不存在
/// 与 merged_ranking 的初始化环。
pub(crate) fn scored_four_key_entries() -> Vec<crate::merged_ranking::ScoredEntry> {
    scored_all_entries()
        .into_iter()
        .filter(|entry| entry.code.len() == 4)
        .collect()
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

    fn core_codes_of(ch: char) -> BTreeSet<String> {
        finalized_char_code_entries()
            .iter()
            .filter(|entry| entry.hanzi().as_char() == ch && entry.is_core_derived())
            .map(|entry| entry.code().to_string())
            .collect()
    }

    #[test]
    fn relation_counts_match_audit() {
        let entries = finalized_char_code_entries();
        assert_eq!(entries.len(), 28_851);
        for (len, expected) in [(2, 9_254), (3, 9_724), (4, 9_873)] {
            assert_eq!(
                entries.iter().filter(|e| e.code().len() == len).count(),
                expected,
                "{len} 码关系数"
            );
        }
        assert_eq!(canonical_char_code_entries().len(), 26_753);
        assert_eq!(
            entries
                .iter()
                .filter(|entry| !entry.is_core_derived())
                .count(),
            2_098
        );
    }

    #[test]
    fn distinct_code_counts_match_audit() {
        let entries = finalized_char_code_entries();
        for (len, expected) in [(2, 414), (3, 5_013), (4, 9_027)] {
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
        for (code, expected) in [("yi", 147), ("jid", 14), ("jumk", 5)] {
            assert_eq!(
                entries
                    .iter()
                    .filter(|e| e.code().to_string() == code)
                    .count(),
                expected,
                "{code} 扇出"
            );
        }
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.code().to_string() == "yi" && entry.is_core_derived())
                .count(),
            136,
            "core yi 扇出保持迁移前不变"
        );
    }

    #[test]
    fn domain_sentinel_code_sets() {
        // 行:三读音 × 单形
        assert_eq!(
            core_codes_of('行'),
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
        assert!(core_codes_of('长').contains("ihp") && core_codes_of('长').contains("vhp"));
        // 贯:三个形码首形键不同,3 码不塌缩(grg/grt/grv)
        assert_eq!(
            core_codes_of('贯'),
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
            core_codes_of('咯'),
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
        // core 语言学读音仍不可机械编码；attested 输入事实独立恢复可达性。
        assert!(core_codes_of('呣').is_empty());
        assert!(core_codes_of('嗯').is_empty());
        assert_eq!(
            codes_of('嗯'),
            BTreeSet::from([
                "en".to_string(),
                "enk".to_string(),
                "enkx".to_string(),
                "ng".to_string(),
                "ngk".to_string(),
                "ngkx".to_string(),
                "og".to_string(),
                "ogk".to_string(),
                "ogkx".to_string(),
                "on".to_string(),
                "onk".to_string(),
                "onkx".to_string(),
            ])
        );
        assert!(codes_of('诶').contains("eiyu"));
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
            // 与词层碰撞的 4 键码使用 merged_ranking 的跨表权重:本表内只是
            // 合并 1..=n 排列的子集,密度不变量由 merged_ranking 测试保证。
            let collided = entries.iter().any(|entry| {
                entry.code() == *code
                    && crate::merged_ranking::merged_weight(
                        entry.code(),
                        &entry.hanzi().as_char().to_string(),
                    )
                    .is_some()
            });
            if collided {
                continue;
            }
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
            .filter(|e| e.code().to_string() == "yi" && e.is_core_derived())
            .collect();
        group.sort_by_key(|a| std::cmp::Reverse(a.rime_weight()));
        // 组首应是万象分数最高的候选,且权重 = 组大小(万象:以 > 一)
        assert_eq!(
            group[0].rime_weight(),
            147,
            "扩展候选追加后旧 core 组首仍保持全组最高权重"
        );
        assert_eq!(group[0].hanzi().as_char(), '以');
        assert_eq!(group[1].hanzi().as_char(), '一');
        // 同组内权重降序 ⟺ 分数降序(tie 时 Unicode 升序)
        for pair in group.windows(2) {
            assert!(pair[0].frequency_score() >= pair[1].frequency_score());
            if pair[0].frequency_score() == pair[1].frequency_score() {
                assert!(pair[0].hanzi() < pair[1].hanzi());
            }
        }
        let all_group: Vec<_> = entries
            .iter()
            .filter(|entry| entry.code().to_string() == "yi")
            .collect();
        let first_extension = all_group
            .iter()
            .position(|entry| !entry.is_core_derived())
            .expect("yi 应有 attested 新增候选");
        assert!(
            all_group[..first_extension]
                .iter()
                .all(|entry| entry.is_core_derived()),
            "新增候选只能追加在旧 core 菜单之后"
        );
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
