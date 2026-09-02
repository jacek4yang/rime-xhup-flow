//! 固定层静态词语编码条目:码推导、(词, 码) 去重、分数聚合、排名与最终化。
//!
//! 本模块是唯一的静态词语管线,Rime 词语词典是它的投影:
//!
//! ```text
//! 规范词语 semantic entries(words.rs)
//!     → 逐字规范读音 → DoublePinyinCode × N → 按字序拼接(2 字 4 键 / 3 字 6 键 / 4 字 8 键)
//!     → 按 (词, 码) 去重,贡献读音序列去重
//!     → 聚合唯一贡献读音序列的万象分数(checked-add)
//!     → 按码分组排名(分数降序,词 Unicode 升序决胜)
//!     → 指派显式 Rime 权重(组内 N..1,正数且唯一)
//!     → 最终化条目集
//! ```
//!
//! P0 不变量:所有 4 键词码与规范单字全码集严格不相交。碰撞过滤已在提取期
//! 按 semantic entry 粒度完成(见 `data/words/README.md`);最终化时再次断言,
//! 使该不变量成为构建级保证而非仅靠测试观察。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use xhup_core::{HanziReading, KeySequence};

use crate::rime::canonical_char_entries;
use crate::words::canonical_word_entries;

/// 一条最终化的静态词语编码关系(模块内投影的事实来源)。
///
/// 字段对 crate 内只读;公共 API 只暴露 [`RimeWordCodeEntry`] 投影,
/// 不暴露万象来源概念或内部排名结构。贡献读音序列证据不随最终条目保存:
/// 测试可直接从 [`crate::words`] 的 semantic entries 独立重建并交叉验证。
pub(crate) struct FinalizedWordCodeEntry {
    word: &'static str,
    code: KeySequence,
    /// 唯一贡献读音序列的万象聚合分数(u64)。
    frequency_score: u64,
    /// 显式 Rime 权重(组内排名 N..1;正数、同码唯一、越大越靠前)。
    rime_weight: u32,
}

impl FinalizedWordCodeEntry {
    pub(crate) fn word(&self) -> &'static str {
        self.word
    }

    pub(crate) fn code(&self) -> &KeySequence {
        &self.code
    }

    pub(crate) fn frequency_score(&self) -> u64 {
        self.frequency_score
    }

    pub(crate) fn rime_weight(&self) -> u32 {
        self.rime_weight
    }
}

/// 最终化静态词语条目集(进程内共享,计算一次)。
///
/// 顺序为词典序列化顺序:码长升序 → 码字典序升序 → 权重降序 → 词升序。
pub(crate) fn finalized_word_code_entries() -> &'static [FinalizedWordCodeEntry] {
    static FINALIZED: OnceLock<Vec<FinalizedWordCodeEntry>> = OnceLock::new();
    FINALIZED.get_or_init(finalize).as_slice()
}

/// 公共投影:一条静态词语编码关系,携带显式 Rime 权重。
///
/// 表示固定层中一个高频词语(2~4 字)的一个可接受静态编码
/// (4/6/8 键,逐字双拼两码按字序拼接)。不携带万象来源概念;
/// 候选顺序由 `weight` 显式表达。
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RimeWordCodeEntry {
    word: String,
    code: KeySequence,
    weight: u32,
}

impl RimeWordCodeEntry {
    /// 该条目对应的词语。
    pub fn word(&self) -> &str {
        &self.word
    }

    /// 该条目对应的静态输入码(4/6/8 键)。
    pub fn code(&self) -> &KeySequence {
        &self.code
    }

    /// 显式 Rime 权重:同码候选中正数且唯一,越大排名越靠前。
    pub fn weight(&self) -> u32 {
        self.weight
    }
}

/// 全部静态词语编码条目(4/6/8 键)的公共投影。
///
/// 与 [`crate::generate_rime_word_dictionary`] 共享同一份最终化条目集:
/// 两者是同一数据的不同视图,不存在第二份推导/排名实现。
///
/// 返回顺序:码长升序、码字典序升序、权重降序、词 Unicode 标量升序。
pub fn canonical_word_code_entries() -> Vec<RimeWordCodeEntry> {
    finalized_word_code_entries()
        .iter()
        .map(|entry| RimeWordCodeEntry {
            word: entry.word().to_string(),
            code: entry.code().clone(),
            weight: entry.rime_weight(),
        })
        .collect()
}

/// 由一条 semantic entry 推导精确 XHUP 词码:逐字双拼两键按字序拼接。
fn derive_code(readings: &[HanziReading]) -> KeySequence {
    let mut keys = Vec::with_capacity(readings.len() * 2);
    for &reading in readings {
        let syllable = reading
            .to_input_syllable()
            .expect("规范词语不变量:读音必然可编码");
        keys.extend_from_slice(syllable.to_double_pinyin_code().as_slice());
    }
    KeySequence::from_keys(&keys).expect("词码非空")
}

/// `(词, 码)` 的中间聚合:唯一贡献读音序列 + 校验和分数。
struct Contribution {
    reading_sequences: BTreeSet<Box<[HanziReading]>>,
    frequency_score: u64,
}

/// 推导原始关系并按 `(词, 码)` 归并贡献读音序列。
fn derive_contributions() -> BTreeMap<(&'static str, KeySequence), Contribution> {
    let mut contributions: BTreeMap<(&'static str, KeySequence), Contribution> = BTreeMap::new();
    for entry in canonical_word_entries() {
        let code = derive_code(entry.readings());
        let contribution = contributions
            .entry((entry.word(), code))
            .or_insert_with(|| Contribution {
                reading_sequences: BTreeSet::new(),
                frequency_score: 0,
            });
        // 归一到同一 (词, 码) 的不同读音序列(如 lo/luo 塌缩)各自只计一次。
        if contribution
            .reading_sequences
            .insert(entry.readings().into())
        {
            contribution.frequency_score = contribution
                .frequency_score
                .checked_add(entry.frequency_score())
                .expect("聚合分数 u64 溢出");
        }
    }
    contributions
}

/// 聚合频率、组内排名、指派权重并按序列化顺序输出最终化条目集。
fn finalize() -> Vec<FinalizedWordCodeEntry> {
    let mut entries: Vec<FinalizedWordCodeEntry> = derive_contributions()
        .into_iter()
        .map(|((word, code), contribution)| FinalizedWordCodeEntry {
            word,
            code,
            frequency_score: contribution.frequency_score,
            rime_weight: 0, // 排名后回填
        })
        .collect();

    // 按码分组排名:聚合分数降序,词 Unicode 标量升序为最终决胜。
    entries.sort_by(|a, b| {
        a.code
            .cmp(&b.code)
            .then(b.frequency_score.cmp(&a.frequency_score))
            .then(a.word.cmp(b.word))
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

    // P0 构建级不变量:4 键词码与规范单字全码集严格不相交。
    // 提取期已按 semantic entry 粒度过滤,此处断言使回归无法静默入库。
    let fullcodes: BTreeSet<String> = canonical_char_entries()
        .iter()
        .map(|entry| entry.code().to_string())
        .collect();
    for entry in &entries {
        if entry.code.len() == 4 {
            assert!(
                !fullcodes.contains(&entry.code.to_string()),
                "P0 不变量被破坏:二字词码 {} 与规范单字全码冲突(词: {})",
                entry.code,
                entry.word
            );
        }
    }

    // 序列化顺序:码长升序 → 码字典序升序 → 权重降序 → 词升序。
    entries.sort_by(|a, b| {
        a.code
            .len()
            .cmp(&b.code.len())
            .then(a.code.cmp(&b.code))
            .then(b.rime_weight.cmp(&a.rime_weight))
            .then(a.word.cmp(b.word))
    });
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use xhup_core::XhupHanzi;

    #[test]
    fn code_lengths_are_4_6_8() {
        for entry in finalized_word_code_entries() {
            assert!(
                matches!(entry.code().len(), 4 | 6 | 8),
                "词码长度应为 4/6/8: {}",
                entry.code()
            );
        }
    }

    #[test]
    fn word_code_pairs_are_unique() {
        let entries = finalized_word_code_entries();
        let mut seen: BTreeSet<(&str, &KeySequence)> = BTreeSet::new();
        for entry in entries {
            assert!(
                seen.insert((entry.word(), entry.code())),
                "(词, 码) 应唯一: {} {}",
                entry.word(),
                entry.code()
            );
        }
    }

    #[test]
    fn two_char_codes_are_disjoint_from_canonical_fullcodes() {
        // P0:最终词层绝不占用规范单字全码
        let fullcodes: BTreeSet<String> = canonical_char_entries()
            .iter()
            .map(|entry| entry.code().to_string())
            .collect();
        for entry in finalized_word_code_entries() {
            if entry.code().len() == 4 {
                assert!(
                    !fullcodes.contains(&entry.code().to_string()),
                    "二字词 {} 的码 {} 与规范全码冲突",
                    entry.word(),
                    entry.code()
                );
            }
        }
    }

    #[test]
    fn weights_are_positive_and_unique_within_code() {
        let entries = finalized_word_code_entries();
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
    fn ranking_is_score_desc_then_word_asc() {
        let entries = finalized_word_code_entries();
        let mut by_code: BTreeMap<&KeySequence, Vec<&FinalizedWordCodeEntry>> = BTreeMap::new();
        for entry in entries {
            by_code.entry(entry.code()).or_default().push(entry);
        }
        for (code, group) in &by_code {
            let mut ranked = group.clone();
            ranked.sort_by_key(|entry| std::cmp::Reverse(entry.rime_weight()));
            for pair in ranked.windows(2) {
                assert!(
                    pair[0].frequency_score >= pair[1].frequency_score,
                    "{code} 权重降序应 ⟺ 分数降序"
                );
                if pair[0].frequency_score == pair[1].frequency_score {
                    assert!(pair[0].word() < pair[1].word(), "{code} 同分决胜为词升序");
                }
            }
        }
    }

    #[test]
    fn collision_filter_is_per_semantic_entry_not_per_word() {
        // 真实 pinned 数据中不存在「同词多读音序列、部分碰撞部分保留」的样本
        // (上游每个词形恰好一个读音序列,审计见 data/words/README.md),
        // 此处用小 fixture 锁定过滤粒度:判定只取决于该 semantic entry 自身
        // 推导的码——碰撞仅排除这一条 (词, 读音序列),与词形无关。
        let fullcodes: BTreeSet<String> = canonical_char_entries()
            .iter()
            .map(|entry| entry.code().to_string())
            .collect();
        let reading_of = |zi: char, spelling: &str| -> HanziReading {
            *XhupHanzi::try_from(zi)
                .unwrap()
                .readings()
                .iter()
                .find(|r| r.as_str() == spelling)
                .unwrap()
        };
        // 「但 dan + 是 shi」推导 djui = 规范全码(「蛋」)→ 该 entry 被排除;
        let collided = derive_code(&[reading_of('但', "dan"), reading_of('是', "shi")]);
        assert_eq!(collided.to_string(), "djui");
        assert!(fullcodes.contains(&collided.to_string()));
        // 「我 wo + 们 men」推导 womf ∉ 规范全码 → 该 entry 保留;
        let retained = derive_code(&[reading_of('我', "wo"), reading_of('们', "men")]);
        assert_eq!(retained.to_string(), "womf");
        assert!(!fullcodes.contains(&retained.to_string()));
        // 同一词形若存在另一读音序列推导出不碰撞的码,该 entry 独立判定保留。
        // 以「长」的多音验证推导是按读音序列独立的:chang/zhang 导出不同码。
        let chang = derive_code(&[reading_of('长', "chang"), reading_of('是', "shi")]);
        let zhang = derive_code(&[reading_of('长', "zhang"), reading_of('是', "shi")]);
        assert_ne!(chang, zhang);
    }

    #[test]
    fn contribution_evidence_matches_aggregated_score() {
        // 内部可验证 semantic reading/frequency 证据:从 canonical semantic
        // entries 独立重建 (词, 码) 的贡献读音序列集与分数和,与最终化条目一致。
        type Evidence = BTreeMap<(String, KeySequence), (BTreeSet<Box<[HanziReading]>>, u64)>;
        let mut evidence: Evidence = BTreeMap::new();
        for semantic in canonical_word_entries() {
            let code = derive_code(semantic.readings());
            let (sequences, score) = evidence
                .entry((semantic.word().to_string(), code))
                .or_insert_with(|| (BTreeSet::new(), 0));
            if sequences.insert(semantic.readings().into()) {
                *score += semantic.frequency_score();
            }
        }
        for entry in finalized_word_code_entries() {
            let (sequences, score) = evidence
                .get(&(entry.word().to_string(), entry.code().clone()))
                .expect("最终关系必然存在 semantic 证据");
            assert!(
                !sequences.is_empty(),
                "{} {} 至少有一个贡献读音序列",
                entry.word(),
                entry.code()
            );
            assert_eq!(
                entry.frequency_score,
                *score,
                "{} {} 聚合分数应等于唯一贡献读音序列分数和",
                entry.word(),
                entry.code()
            );
        }
    }

    #[test]
    fn serialization_order_is_total_and_deterministic() {
        let entries = finalized_word_code_entries();
        for pair in entries.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            assert!(
                (
                    a.code().len(),
                    a.code(),
                    u32::MAX - a.rime_weight(),
                    a.word()
                ) < (
                    b.code().len(),
                    b.code(),
                    u32::MAX - b.rime_weight(),
                    b.word()
                ),
                "序列化顺序应严格递增"
            );
        }
    }
}
