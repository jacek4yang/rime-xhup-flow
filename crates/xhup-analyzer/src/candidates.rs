//! 词语简码候选枚举:逐字 F/I 投影,版本化候选语法。
//!
//! 每个字在 shortcut 中只能选择
//!
//! ```text
//! F = Full    完整双拼两键
//! I = Initial 只取双拼码第一键
//! ```
//!
//! 结构合法性与长度策略严格分层:
//!
//! - [`CandidateGrammar`] 只回答「哪些 F/I 模式结构合法」。两个语法的共同
//!   invariant 都是「至少含一个 I」(all-F 等于完整码,不是 shortcut):
//!   `LegacyAnyFiV1` 接受任意含 I 的 F/I 组合(PR #21/#22 冻结语法);
//!   `MonotoneSuffixInitialsV2` 额外要求单调后缀缩写 `F* I*`(一旦 I 出现,
//!   后续不得再 F)。
//! - [`CandidateEnumerationSpec`] = grammar + 枚举期最小码长。历史
//!   PR #21/#22 的 `len >= 3` 过滤属于冻结枚举规格,不属于语法本身;
//!   Monotone V2 的理论全集允许 2-key 候选,production 最短长度由
//!   production policy(见 `production_fixed_first`)在优化前过滤。
//!
//! 候选直接从最终完整词码推导(每两键为一个字的双拼码),不需要读音层。
//! `LegacyAnyFiV1` 枚举保持位掩码实现与 dedup/tie-break 语义逐字节不变
//! (PR #22 canonical 复现依赖);`MonotoneSuffixInitialsV2` 直接生成
//! `F^(N-k) I^k`(k = 1..N):每个 k 的 shortcut 长度 `2N-k` 互不相同,
//! 不同合法模式不可能投影出相同码,无需 dedup。
//!
//! 保持原字序;候选按 (词, shortcut 码) 去重(仅 Legacy 语法需要)。

use std::collections::BTreeMap;

use xhup_core::KeySequence;
use xhup_generator::WordCodeAnalysisEntry;

/// 词语简码候选语法版本(语义身份;不用字符串/布尔/魔法标志)。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum CandidateGrammar {
    /// 任意含 I 的 F/I 组合(PR #21/#22 冻结语法)。
    ///
    /// 冻结含义:枚举期同时应用 `len >= 3` 过滤(见
    /// [`CandidateEnumerationSpec::LEGACY_V1_FROZEN`])—— 这是历史行为的一
    /// 部分,ZERO_REGRESSION canonical 复现依赖它。新 production policy
    /// 不应再把该语法用于新的候选生成。
    LegacyAnyFiV1,
    /// 单调后缀缩写:`F* I*` 且至少一个 I(未来/研究语法)。
    ///
    /// 结构含义:一旦某字被缩写(I),其后所有字都必须缩写 —— 缩写链
    /// `完整码 → 缩短一档 → …` 单调递减,符合人类肌肉记忆的方向性。
    /// 理论全集允许 2-key 候选(如 `时间 → uj/II`);production 是否采用
    /// 由 policy 决定,语法不擦除。
    MonotoneSuffixInitialsV2,
}

impl CandidateGrammar {
    /// canonical 标识串(用于 TSV 头、报告与审计;非语义身份,仅展示)。
    pub fn label(self) -> &'static str {
        match self {
            CandidateGrammar::LegacyAnyFiV1 => "legacy-any-fi-v1",
            CandidateGrammar::MonotoneSuffixInitialsV2 => "monotone-suffix-initials-v2",
        }
    }

    /// 模式是否属于本语法。
    ///
    /// 共同 invariant:至少含一个 I(all-F 等于完整码,不是 shortcut)。
    /// `LegacyAnyFiV1` 仅此一条;`MonotoneSuffixInitialsV2` 额外要求
    /// `F* I*` 单调性。
    pub fn accepts(self, mode: &ShortcutMode) -> bool {
        match self {
            CandidateGrammar::LegacyAnyFiV1 => mode.has_initial(),
            CandidateGrammar::MonotoneSuffixInitialsV2 => {
                mode.has_initial() && mode.is_monotone_suffix()
            }
        }
    }
}

/// 候选枚举规格:结构语法 + 枚举期最小码长。
///
/// 语法回答「什么结构合法」,本规格回答「枚举时保留多长的候选」。两者
/// 分层:语法不表达长度策略,长度策略不改变语法身份。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CandidateEnumerationSpec {
    /// 候选语法。
    pub grammar: CandidateGrammar,
    /// 枚举期最小 shortcut 码长(更短的模式在枚举期直接跳过)。
    pub min_length: usize,
}

impl CandidateEnumerationSpec {
    /// PR #21/#22 冻结枚举规格:`LegacyAnyFiV1` × 最短 3 键。
    ///
    /// 历史上 1/2 键空间保留给一级简码与单字;该过滤属于冻结枚举行为,
    /// ZERO_REGRESSION canonical 复现(44,448 行)依赖此规格逐字节一致。
    pub const LEGACY_V1_FROZEN: Self = Self {
        grammar: CandidateGrammar::LegacyAnyFiV1,
        min_length: 3,
    };

    /// Monotone V2 理论全集规格:语法层全部合法模式(允许 2-key 候选)。
    ///
    /// 结构上最短的合法 shortcut 是全缩写模式 `II..I`(N 字词 N 键,N ≥ 2),
    /// 故 `min_length: 2` 即不擦除任何理论模式。production 最短长度由
    /// `production_fixed_first::PRODUCTION_MIN_SHORTCUT_LENGTH` 在优化前
    /// 作为 policy 过滤,不在此表达。
    pub const MONOTONE_V2_THEORETICAL: Self = Self {
        grammar: CandidateGrammar::MonotoneSuffixInitialsV2,
        min_length: 2,
    };
}

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

    /// 是否至少含一个 I(all-F 等于完整码,不是 shortcut)。
    pub fn has_initial(&self) -> bool {
        self.0.iter().any(|mode| matches!(mode, Mode::Initial))
    }

    /// 是否为单调后缀缩写 `F* I*`(一旦 I 出现,后续不得再 F)。
    ///
    /// 与 generator 侧的 F/I 校验是同一个小不变式的独立实现(generator
    /// 不依赖 analyzer);两侧行为由共享测试语义锁定。
    pub fn is_monotone_suffix(&self) -> bool {
        self.0
            .windows(2)
            .all(|pair| !(pair[0] == Mode::Initial && pair[1] == Mode::Full))
    }
}

impl ShortcutMode {
    /// 测试构造:单 F 模式(仅供 synthetic 测试;对集成测试可见)。
    #[doc(hidden)]
    pub fn for_test() -> Self {
        ShortcutMode(Box::new([Mode::Full]))
    }
}

/// 一条词语简码候选。
#[derive(Clone)]
pub struct ShortcutCandidate {
    shortcut_code: KeySequence,
    mode: ShortcutMode,
}

impl ShortcutCandidate {
    /// shortcut 码(严格短于完整码)。
    pub fn shortcut_code(&self) -> &KeySequence {
        &self.shortcut_code
    }

    /// 逐字投影模式。
    pub fn mode(&self) -> &ShortcutMode {
        &self.mode
    }
}

/// 一个分析目标:identity 为 `(词, 完整码)`,支持未来多音词。
#[derive(Clone)]
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

    /// 全部合法 shortcut 候选(已按枚举规格过滤)。
    pub fn candidates(&self) -> &[ShortcutCandidate] {
        &self.candidates
    }

    /// 该候选相对完整码节省的键数。
    pub fn keys_saved(&self, candidate: &ShortcutCandidate) -> usize {
        self.full_code.len() - candidate.shortcut_code().len()
    }

    /// 按谓词保留候选(用于 incremental production 优化前的 candidate
    /// universe 收缩;被移除的候选不参与 greedy assignment)。
    pub fn retain_candidates(&mut self, mut keep: impl FnMut(&ShortcutCandidate) -> bool) {
        self.candidates.retain(|candidate| keep(candidate));
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

impl WordTarget {
    /// 测试构造:带候选的 target(候选码/模式不要求与完整码投影一致,
    /// 仅用于 optimizer/universe 机制的 synthetic 测试;对集成测试可见)。
    #[doc(hidden)]
    pub fn with_candidates_for_test(
        word: &str,
        full_code: KeySequence,
        frequency_score: u64,
        candidates: Vec<ShortcutCandidate>,
    ) -> Self {
        WordTarget {
            word: word.to_string(),
            full_code,
            frequency_score,
            candidates,
        }
    }
}

impl ShortcutCandidate {
    /// 测试构造:任意码 + 单 F 模式(仅供 synthetic 测试;对集成测试可见)。
    #[doc(hidden)]
    pub fn for_test(shortcut_code: KeySequence) -> Self {
        ShortcutCandidate {
            shortcut_code,
            mode: ShortcutMode::for_test(),
        }
    }
}

/// 候选枚举统计。
#[derive(Clone, Debug, Default)]
pub struct EnumerationStats {
    /// 枚举期(规格过滤后)理论候选数。
    pub theoretical: usize,
    /// 去重后实际候选数(Monotone V2 恒等于 theoretical)。
    pub actual: usize,
    /// 按 shortcut 码长的候选数。
    pub by_length: BTreeMap<usize, usize>,
    /// 按投影模式的候选数(如 `FI`、`IF`)。
    pub by_pattern: BTreeMap<String, usize>,
}

impl EnumerationStats {
    /// 去重删除的候选数(Monotone V2 恒为 0:不同合法模式码长互不相同)。
    pub fn dedup_removed(&self) -> usize {
        self.theoretical - self.actual
    }
}

/// 枚举单个词的全部合法 shortcut 候选(按枚举规格)。
///
/// `LegacyAnyFiV1`:位掩码枚举 F/I 组合(冻结实现),同一 (词, 码) 由多个
/// 模式投影得到时保留 transitions 最少的模式(mode-complexity 系数非负下
/// 的最优选择),并列取模式字典序最小者 —— 结果与枚举顺序无关。
///
/// `MonotoneSuffixInitialsV2`:直接生成 `F^(N-k) I^k`(k = 1..N)。每个 k
/// 的 shortcut 长度 `2N-k` 互不相同,不同合法模式不可能投影出相同码,
/// 无需 dedup/tie-break。候选按 k 升序排列(完整码 → 最短缩写的单调
/// 缩写链)。
fn enumerate_one_with_spec(
    full_code: &KeySequence,
    spec: CandidateEnumerationSpec,
) -> (Vec<ShortcutCandidate>, usize) {
    let char_count = full_code.len() / 2;
    let keys = full_code.as_slice();
    let (chunks, _) = keys.as_chunks::<2>();
    match spec.grammar {
        CandidateGrammar::MonotoneSuffixInitialsV2 => {
            // 直接枚举单调后缀缩写:k = 1..N 个后缀字缩写(I),其余 Full。
            let mut theoretical = 0usize;
            let mut candidates = Vec::with_capacity(char_count);
            for initial_count in 1..=char_count {
                let length = char_count * 2 - initial_count;
                if length < spec.min_length {
                    continue;
                }
                theoretical += 1;
                let mut shortcut_keys = Vec::with_capacity(length);
                let mut modes = Vec::with_capacity(char_count);
                for (index, chunk) in chunks.iter().enumerate() {
                    if index >= char_count - initial_count {
                        shortcut_keys.push(chunk[0]);
                        modes.push(Mode::Initial);
                    } else {
                        shortcut_keys.extend_from_slice(chunk);
                        modes.push(Mode::Full);
                    }
                }
                candidates.push(ShortcutCandidate {
                    shortcut_code: KeySequence::from_keys(&shortcut_keys).expect("shortcut 非空"),
                    mode: ShortcutMode(modes.into_boxed_slice()),
                });
            }
            (candidates, theoretical)
        }
        CandidateGrammar::LegacyAnyFiV1 => {
            let mut theoretical = 0usize;
            let mut by_code: BTreeMap<KeySequence, ShortcutMode> = BTreeMap::new();
            // 位掩码枚举 F/I 组合:bit i 置位 = 第 i 字取 Initial。
            // mask 0(all-F)等于完整码,直接排除;枚举顺序固定保证确定性。
            for mask in 1usize..(1usize << char_count) {
                let initial_count = mask.count_ones() as usize;
                let length = char_count * 2 - initial_count;
                if length < spec.min_length {
                    continue;
                }
                theoretical += 1;
                let mut shortcut_keys = Vec::with_capacity(length);
                let mut modes = Vec::with_capacity(char_count);
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
                        if (mode.transitions(), mode.pattern())
                            < (current.transitions(), current.pattern())
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
    }
}

/// 枚举全部词语目标的合法 shortcut 候选(按枚举规格)。
///
/// Monotone V2 理论全集不变量:每个词的理论候选数 = 字数,且去重删除
/// 恒为 0(不同 k 的码长互不相同);canonical word universe 中失败即
/// 硬断言(STOP)。
pub fn enumerate_targets_with_spec(
    words: &[WordCodeAnalysisEntry],
    spec: CandidateEnumerationSpec,
) -> (Vec<WordTarget>, EnumerationStats) {
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
        let (candidates, theoretical) = enumerate_one_with_spec(&full_code, spec);
        if spec.grammar == CandidateGrammar::MonotoneSuffixInitialsV2 && spec.min_length <= 2 {
            // 理论全集不变量:N 个模式全部保留,无需 dedup。
            assert_eq!(
                candidates.len(),
                char_count,
                "Monotone V2 理论全集候选数应恰为字数: {word}"
            );
            assert_eq!(theoretical, char_count);
        }
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

/// 枚举全部词语目标的合法 shortcut 候选(PR #21/#22 冻结语法)。
///
/// 显式等价于 [`enumerate_targets_with_spec`] + [`CandidateEnumerationSpec::LEGACY_V1_FROZEN`];
/// 新的分析/研究应显式选择 [`CandidateEnumerationSpec::MONOTONE_V2_THEORETICAL`]。
pub fn enumerate_targets(words: &[WordCodeAnalysisEntry]) -> (Vec<WordTarget>, EnumerationStats) {
    enumerate_targets_with_spec(words, CandidateEnumerationSpec::LEGACY_V1_FROZEN)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates_of(full: &str, spec: CandidateEnumerationSpec) -> Vec<(String, String)> {
        let full: KeySequence = full.parse().unwrap();
        enumerate_one_with_spec(&full, spec)
            .0
            .iter()
            .map(|c| (c.shortcut_code().to_string(), c.mode().pattern()))
            .collect()
    }

    /// 语法接受性:共同 invariant 是至少一个 I;Monotone V2 额外要求
    /// `F* I*` 单调性。
    #[test]
    fn grammar_accepts_requires_at_least_one_initial() {
        let build = |pattern: &str| {
            ShortcutMode(
                pattern
                    .chars()
                    .map(|c| match c {
                        'F' => Mode::Full,
                        'I' => Mode::Initial,
                        other => panic!("非法模式字符: {other}"),
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            )
        };
        // Legacy valid:任意含 I 的 F/I 组合。
        for pattern in ["FI", "IF", "II", "FIF", "IIF", "III", "FFI", "IIII"] {
            assert!(
                CandidateGrammar::LegacyAnyFiV1.accepts(&build(pattern)),
                "Legacy 应接受 {pattern}"
            );
        }
        // Legacy invalid:all-F(等于完整码)。
        for pattern in ["F", "FF", "FFF", "FFFF"] {
            assert!(
                !CandidateGrammar::LegacyAnyFiV1.accepts(&build(pattern)),
                "Legacy 应拒绝 {pattern}"
            );
        }
        // Monotone valid:`F* I*` 且至少一个 I。
        for pattern in [
            "FI", "II", "FFI", "FII", "III", "FFFI", "FFII", "FIII", "IIII",
        ] {
            assert!(
                CandidateGrammar::MonotoneSuffixInitialsV2.accepts(&build(pattern)),
                "Monotone 应接受 {pattern}"
            );
        }
        // Monotone invalid:all-F,或 I 之后再次出现 F。
        for pattern in [
            "F", "FF", "FFF", "FFFF", "IF", "IFI", "IFF", "IIF", "FIF", "IIIF", "IFII", "IIFI",
        ] {
            assert!(
                !CandidateGrammar::MonotoneSuffixInitialsV2.accepts(&build(pattern)),
                "Monotone 应拒绝 {pattern}"
            );
        }
    }

    /// Legacy 冻结行为:「时间」候选恰为 uij(FI)与 ujm(IF)。
    #[test]
    fn legacy_time_sentinel_fi_and_if() {
        assert_eq!(
            candidates_of("uijm", CandidateEnumerationSpec::LEGACY_V1_FROZEN),
            vec![
                ("uij".to_string(), "FI".to_string()),
                ("ujm".to_string(), "IF".to_string())
            ]
        );
    }

    /// Monotone V2:「时间」候选恰为 uij(FI)与 uj(II);ujm(IF)结构性非法。
    #[test]
    fn monotone_time_sentinel_fi_and_ii() {
        assert_eq!(
            candidates_of("uijm", CandidateEnumerationSpec::MONOTONE_V2_THEORETICAL),
            vec![
                ("uij".to_string(), "FI".to_string()),
                ("uj".to_string(), "II".to_string())
            ]
        );
    }

    /// Monotone V2 直接枚举:4 字词的缩写链恰为 FFFI/FFII/FIII/IIII。
    #[test]
    fn monotone_ladder_for_four_char_word() {
        // chunks (u,i)(j,m)(a,a)(b,b):k=1..4 依次 FFFI/FFII/FIII/IIII,
        // 码长 7/6/5/4 互不相同。
        assert_eq!(
            candidates_of(
                "uijmaabb",
                CandidateEnumerationSpec::MONOTONE_V2_THEORETICAL
            ),
            vec![
                ("uijmaab".to_string(), "FFFI".to_string()),
                ("uijmab".to_string(), "FFII".to_string()),
                ("uijab".to_string(), "FIII".to_string()),
                ("ujab".to_string(), "IIII".to_string()),
            ]
        );
    }

    /// 同一 shortcut 码可由多个 Legacy F/I 模式投影得到时,保留 transitions
    /// 最少者,与枚举顺序无关(冻结 dedup/tie-break)。
    #[test]
    fn duplicate_shortcut_code_keeps_minimum_transition_mode() {
        // c1=(u,i) c2=(j,a) c3=(a,a):IIF 与 IFI 都投影为 "ujaa";
        // IIF transitions=1,IFI transitions=2 → 必须保留 IIF。
        let full: KeySequence = "uijaaa".parse().unwrap();
        let (candidates, _) =
            enumerate_one_with_spec(&full, CandidateEnumerationSpec::LEGACY_V1_FROZEN);
        let entry = candidates
            .iter()
            .find(|c| c.shortcut_code().to_string() == "ujaa")
            .expect("ujaa 候选必然存在");
        assert_eq!(entry.mode().pattern(), "IIF");
    }

    /// 枚举确定性:两次运行产出逐条一致(码与模式);两个语法都覆盖。
    #[test]
    fn enumeration_is_deterministic() {
        for spec in [
            CandidateEnumerationSpec::LEGACY_V1_FROZEN,
            CandidateEnumerationSpec::MONOTONE_V2_THEORETICAL,
        ] {
            let full: KeySequence = "uijaaa".parse().unwrap();
            let project =
                |c: &ShortcutCandidate| (c.shortcut_code().to_string(), c.mode().pattern());
            let first: Vec<(String, String)> = enumerate_one_with_spec(&full, spec)
                .0
                .iter()
                .map(project)
                .collect();
            let second: Vec<(String, String)> = enumerate_one_with_spec(&full, spec)
                .0
                .iter()
                .map(project)
                .collect();
            assert_eq!(first, second);
        }
    }
}
