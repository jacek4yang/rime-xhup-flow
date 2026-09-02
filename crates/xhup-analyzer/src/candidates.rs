//! 词语简码候选枚举:逐字 F/I 投影。
//!
//! frozen rule:词语 shortcut 中每个字只能选择
//!
//! ```text
//! F = Full    完整双拼两键
//! I = Initial 只取双拼码第一键
//! ```
//!
//! 保持原字序;删除 all-F(等于完整码,不是 shortcut);删除长度 < 3
//! (1/2 键空间保留给一级简码与单字);按 (词, shortcut 码) 去重。
//! 候选直接从最终完整词码推导(每两键为一个字的双拼码),不需要读音层。

use std::collections::BTreeMap;

use xhup_core::KeySequence;
use xhup_generator::WordCodeAnalysisEntry;

/// 词语 shortcut 的最小长度(1/2 键空间保留给一级简码与单字)。
pub const MIN_SHORTCUT_LENGTH: usize = 3;

/// 单个字在 shortcut 中的投影方式。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Mode {
    /// Full:完整双拼两键。
    Full,
    /// Initial:只取双拼码第一键。
    Initial,
}

/// 一个词 shortcut 的逐字投影模式(如 `FI`、`IF`、`FII`)。
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ShortcutMode(Box<[Mode]>);

impl ShortcutMode {
    /// 逐字模式切片。
    pub fn modes(&self) -> &[Mode] {
        &self.0
    }

    /// 模式中的 F/I 切换次数(如 `FI` = 1,`FII` = 1,`FIF` = 2)。
    pub fn transitions(&self) -> usize {
        self.0.windows(2).filter(|pair| pair[0] != pair[1]).count()
    }

    /// 模式字符串(如 `FI`)。
    pub fn pattern(&self) -> String {
        self.0
            .iter()
            .map(|mode| match mode {
                Mode::Full => 'F',
                Mode::Initial => 'I',
            })
            .collect()
    }
}

/// 一条词语简码候选。
pub struct ShortcutCandidate {
    shortcut_code: KeySequence,
    mode: ShortcutMode,
}

impl ShortcutCandidate {
    /// shortcut 码(长度 ≥ 3 且 < 完整码长)。
    pub fn shortcut_code(&self) -> &KeySequence {
        &self.shortcut_code
    }

    /// 逐字投影模式。
    pub fn mode(&self) -> &ShortcutMode {
        &self.mode
    }
}

/// 一个分析目标:identity 为 `(词, 完整码)`,支持未来多音词。
pub struct WordTarget {
    word: String,
    full_code: KeySequence,
    frequency_score: u64,
    candidates: Vec<ShortcutCandidate>,
}

impl WordTarget {
    /// 词语。
    pub fn word(&self) -> &str {
        &self.word
    }

    /// 完整码(4/6/8 键)。
    pub fn full_code(&self) -> &KeySequence {
        &self.full_code
    }

    /// 万象聚合频率分数。
    pub fn frequency_score(&self) -> u64 {
        self.frequency_score
    }

    /// 全部合法 shortcut 候选(已去重)。
    pub fn candidates(&self) -> &[ShortcutCandidate] {
        &self.candidates
    }

    /// 该候选相对完整码节省的键数。
    pub fn keys_saved(&self, candidate: &ShortcutCandidate) -> usize {
        self.full_code.len() - candidate.shortcut_code().len()
    }
}

#[cfg(test)]
impl WordTarget {
    /// 测试构造:无候选的裸 target。
    pub(crate) fn new_for_test(word: &str, full_code: KeySequence, frequency_score: u64) -> Self {
        WordTarget {
            word: word.to_string(),
            full_code,
            frequency_score,
            candidates: Vec::new(),
        }
    }
}

/// 候选枚举统计。
#[derive(Clone, Debug, Default)]
pub struct EnumerationStats {
    /// 去重前候选数(模式组合理论值)。
    pub theoretical: usize,
    /// 去重后实际候选数。
    pub actual: usize,
    /// 按 shortcut 码长的候选数。
    pub by_length: BTreeMap<usize, usize>,
    /// 按投影模式的候选数(如 `FI`、`IF`)。
    pub by_pattern: BTreeMap<String, usize>,
}

impl EnumerationStats {
    /// 去重删除的候选数。
    pub fn dedup_removed(&self) -> usize {
        self.theoretical - self.actual
    }
}

/// 枚举单个词的全部合法 shortcut 候选。
///
/// 返回 (去重后候选, 去重前理论候选数)。不同模式可能投影出相同码(相邻键
/// 相同):同一 (词, 码) 保留 transitions 最少的模式(mode-complexity 系数
/// 非负下的最优选择),并列取模式字典序最小者 —— 结果与枚举顺序无关。
fn enumerate_one(full_code: &KeySequence) -> (Vec<ShortcutCandidate>, usize) {
    let char_count = full_code.len() / 2;
    let keys = full_code.as_slice();
    let mut theoretical = 0usize;
    let mut by_code: BTreeMap<KeySequence, ShortcutMode> = BTreeMap::new();
    // 位掩码枚举 F/I 组合:bit i 置位 = 第 i 字取 Initial。
    // mask 0(all-F)等于完整码,直接排除;枚举顺序固定保证确定性。
    for mask in 1usize..(1usize << char_count) {
        let initial_count = mask.count_ones() as usize;
        let length = char_count * 2 - initial_count;
        if length < MIN_SHORTCUT_LENGTH {
            continue;
        }
        theoretical += 1;
        let mut shortcut_keys = Vec::with_capacity(length);
        let mut modes = Vec::with_capacity(char_count);
        let (chunks, _) = keys.as_chunks::<2>();
        for (index, chunk) in chunks.iter().enumerate() {
            if mask & (1 << index) != 0 {
                shortcut_keys.push(chunk[0]);
                modes.push(Mode::Initial);
            } else {
                shortcut_keys.extend_from_slice(chunk);
                modes.push(Mode::Full);
            }
        }
        let shortcut_code = KeySequence::from_keys(&shortcut_keys).expect("shortcut 非空");
        let mode = ShortcutMode(modes.into_boxed_slice());
        match by_code.entry(shortcut_code) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(mode);
            }
            std::collections::btree_map::Entry::Occupied(mut slot) => {
                let current = slot.get();
                if (mode.transitions(), mode.pattern()) < (current.transitions(), current.pattern())
                {
                    slot.insert(mode);
                }
            }
        }
    }
    let candidates = by_code
        .into_iter()
        .map(|(shortcut_code, mode)| ShortcutCandidate {
            shortcut_code,
            mode,
        })
        .collect();
    (candidates, theoretical)
}

/// 枚举全部词语目标的合法 shortcut 候选。
///
/// P0 哨兵:「时间」(full `uijm`)的候选必须恰好是 `uij`(FI)与 `ujm`(IF)。
pub fn enumerate_targets(words: &[WordCodeAnalysisEntry]) -> (Vec<WordTarget>, EnumerationStats) {
    let mut targets = Vec::with_capacity(words.len());
    let mut stats = EnumerationStats::default();
    for entry in words {
        let word = entry.word().to_string();
        let full_code = entry.code().clone();
        let char_count = word.chars().count();
        assert_eq!(
            full_code.len(),
            char_count * 2,
            "词码长度应为字数两倍: {word} {full_code}"
        );
        let (candidates, theoretical) = enumerate_one(&full_code);
        stats.theoretical += theoretical;
        for candidate in &candidates {
            *stats
                .by_length
                .entry(candidate.shortcut_code().len())
                .or_default() += 1;
            *stats
                .by_pattern
                .entry(candidate.mode().pattern())
                .or_default() += 1;
        }
        stats.actual += candidates.len();
        targets.push(WordTarget {
            word,
            full_code,
            frequency_score: entry.frequency_score(),
            candidates,
        });
    }
    (targets, stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 同一 shortcut 码可由多个 F/I 模式投影得到时,保留 transitions 最少者,
    /// 与枚举顺序无关。
    #[test]
    fn duplicate_shortcut_code_keeps_minimum_transition_mode() {
        // c1=(u,i) c2=(j,a) c3=(a,a):IIF 与 IFI 都投影为 "ujaa";
        // IIF transitions=1,IFI transitions=2 → 必须保留 IIF。
        let full: KeySequence = "uijaaa".parse().unwrap();
        let (candidates, _) = enumerate_one(&full);
        let entry = candidates
            .iter()
            .find(|c| c.shortcut_code().to_string() == "ujaa")
            .expect("ujaa 候选必然存在");
        assert_eq!(entry.mode().pattern(), "IIF");
    }

    /// 枚举确定性:两次运行产出逐条一致(码与模式)。
    #[test]
    fn enumeration_is_deterministic() {
        let full: KeySequence = "uijaaa".parse().unwrap();
        let first: Vec<(String, String)> = enumerate_one(&full)
            .0
            .iter()
            .map(|c| (c.shortcut_code().to_string(), c.mode().pattern()))
            .collect();
        let second: Vec<(String, String)> = enumerate_one(&full)
            .0
            .iter()
            .map(|c| (c.shortcut_code().to_string(), c.mode().pattern()))
            .collect();
        assert_eq!(first, second);
    }
}
