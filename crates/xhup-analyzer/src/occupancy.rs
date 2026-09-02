//! 生产码位占用:从 generator 的公共/分析投影重建 exact-code 空间。
//!
//! 本模块是 optimizer 的全部"现状"事实来源,显式区分两种语义:
//!
//! - [`CodeOccupancy::build_baseline_fixed`]:一级简码(1 键)+ 静态单字
//!   (2/3/4 码)+ 静态词语(4/6/8 键),即 optimizer 的历史/优化 baseline,
//!   **不包含**已入库的词语简码层。production selection 永远基于它。
//! - [`CodeOccupancy::build_current_production`]:baseline + 已入库的
//!   高稳健零冲突词语简码层(`data/shortcuts/word_zero_regression.tsv`),
//!   即当前真实生产占用。审计与后续优化必须能看到这些已占用码位。
//!
//! 候选顺序由显式 Rime 权重降序表达。所有统计都从真实 canonical data 现算,
//! 不硬编码行数。

use std::collections::BTreeMap;

use xhup_core::KeySequence;
use xhup_generator::{
    canonical_level1_shortcuts, canonical_word_shortcut_entries, char_code_analysis_entries,
    word_code_analysis_entries,
};

/// 现有候选的来源层。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum CandidateSource {
    /// 一级简码固定层(1 键)。
    Level1Shortcut,
    /// 静态单字层(2/3/4 码;4 码即规范全码)。
    CharCode,
    /// 静态高频词语层(4/6/8 键)。
    FixedWord,
    /// 已入库的高稳健零冲突词语简码层(3~7 键 alias)。
    WordShortcut,
}

impl CandidateSource {
    /// 报告用稳定标签。
    pub fn label(self) -> &'static str {
        match self {
            CandidateSource::Level1Shortcut => "level1_shortcut",
            CandidateSource::CharCode => "char_code",
            CandidateSource::FixedWord => "fixed_word",
            CandidateSource::WordShortcut => "word_shortcut",
        }
    }
}

/// 一个码位上已有候选的碰撞类型分类。
///
/// 词语简码长度至少为 3,正常不会命中 1/2 键空间,但类型系统保持完整。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum CollisionClass {
    /// 该 exact code 当前完全空闲。
    None,
    /// 命中一级简码(1 键)。
    Level1,
    /// 命中 2 码单字。
    Char2Key,
    /// 命中 3 码单字。
    Char3Key,
    /// 命中 4 码规范全码单字。
    FullCodeChar,
    /// 命中固定词语(4/6/8 键)。
    FixedWord,
    /// 命中已入库的词语简码(3~7 键)。
    WordShortcut,
    /// 同时命中多个来源层。
    Multiple,
}

impl CollisionClass {
    /// 报告用稳定标签。
    pub fn label(self) -> &'static str {
        match self {
            CollisionClass::None => "NONE",
            CollisionClass::Level1 => "LEVEL1",
            CollisionClass::Char2Key => "2KEY_CHAR",
            CollisionClass::Char3Key => "3KEY_CHAR",
            CollisionClass::FullCodeChar => "FULLCODE_CHAR",
            CollisionClass::FixedWord => "FIXED_WORD",
            CollisionClass::WordShortcut => "WORD_SHORTCUT",
            CollisionClass::Multiple => "MULTIPLE",
        }
    }
}

/// 一个码位上的一条现有候选。
///
/// `rank` 为组内 1 起始名次(显式权重降序,权重同码唯一,故名次确定)。
pub struct ExistingCandidate {
    text: String,
    source: CandidateSource,
    rank: u32,
    rime_weight: u32,
    frequency_score: u64,
}

impl ExistingCandidate {
    /// 候选文本(单字或词语)。
    pub fn text(&self) -> &str {
        &self.text
    }

    /// 来源层。
    pub fn source(&self) -> CandidateSource {
        self.source
    }

    /// 组内名次(1 起始,权重降序)。
    pub fn rank(&self) -> u32 {
        self.rank
    }

    /// 显式 Rime 权重。
    pub fn rime_weight(&self) -> u32 {
        self.rime_weight
    }

    /// 万象聚合频率分数(一级简码无频率证据,恒为 0;词语简码复制其
    /// `(词, 完整码)` 对应的真实词频分数)。
    pub fn frequency_score(&self) -> u64 {
        self.frequency_score
    }
}

/// 固定层 exact-code 占用表(baseline 或含词语简码层的当前生产占用,
/// 由构建入口决定)。
pub struct CodeOccupancy {
    /// 码 → 候选组(组内按权重降序,rank 已回填)。
    groups: BTreeMap<KeySequence, Vec<ExistingCandidate>>,
}

impl CodeOccupancy {
    /// 重建优化 baseline 固定层占用:一级简码 + 静态单字 + 静态词语,
    /// **不包含**已入库的词语简码层。production selection 永远基于它。
    pub fn build_baseline_fixed() -> Self {
        Self::build_impl(false)
    }

    /// 重建当前真实生产占用:baseline fixed + 已入库的高稳健零冲突词语
    /// 简码层。词语简码候选携带其 `(词, 完整码)` 对应的真实词频证据。
    pub fn build_current_production() -> Self {
        Self::build_impl(true)
    }

    fn build_impl(with_word_shortcuts: bool) -> Self {
        let mut groups: BTreeMap<KeySequence, Vec<ExistingCandidate>> = BTreeMap::new();
        let mut push = |code: &KeySequence,
                        text: String,
                        source: CandidateSource,
                        rime_weight: u32,
                        frequency_score: u64| {
            groups
                .entry(code.clone())
                .or_default()
                .push(ExistingCandidate {
                    text,
                    source,
                    rank: 0, // 排序后回填
                    rime_weight,
                    frequency_score,
                });
        };

        for entry in canonical_level1_shortcuts() {
            let code = KeySequence::from_keys(&[entry.key()]).expect("一键非空");
            push(
                &code,
                entry.hanzi().as_char().to_string(),
                CandidateSource::Level1Shortcut,
                1,
                0,
            );
        }
        for entry in char_code_analysis_entries() {
            push(
                entry.code(),
                entry.hanzi().as_char().to_string(),
                CandidateSource::CharCode,
                entry.rime_weight(),
                entry.frequency_score(),
            );
        }
        for entry in word_code_analysis_entries() {
            push(
                entry.code(),
                entry.word().to_string(),
                CandidateSource::FixedWord,
                entry.rime_weight(),
                entry.frequency_score(),
            );
        }
        if with_word_shortcuts {
            // (词, 完整码) → 真实词频分数;TSV 解析已保证每个 (词, 完整码)
            // 在固定词层存在,查不到即数据损坏,直接 panic。
            let word_entries = word_code_analysis_entries();
            let word_scores: BTreeMap<(&str, &KeySequence), u64> = word_entries
                .iter()
                .map(|entry| ((entry.word(), entry.code()), entry.frequency_score()))
                .collect();
            for entry in canonical_word_shortcut_entries() {
                let frequency_score = *word_scores
                    .get(&(entry.word(), entry.full_code()))
                    .expect("词语简码的 (词, 完整码) 必须存在于固定词层");
                push(
                    entry.shortcut_code(),
                    entry.word().to_string(),
                    CandidateSource::WordShortcut,
                    1,
                    frequency_score,
                );
            }
        }

        // 组内排序:权重降序;权重同码唯一,文本升序仅为确定性兜底。
        for group in groups.values_mut() {
            group.sort_by(|a, b| b.rime_weight.cmp(&a.rime_weight).then(a.text.cmp(&b.text)));
            for (index, candidate) in group.iter_mut().enumerate() {
                candidate.rank = u32::try_from(index + 1).expect("组内名次超出 u32");
            }
        }
        CodeOccupancy { groups }
    }

    /// 查询某个 exact code 的现有候选组(权重降序);空闲码返回 `None`。
    pub fn group(&self, code: &KeySequence) -> Option<&[ExistingCandidate]> {
        self.groups.get(code).map(Vec::as_slice)
    }

    /// 现有候选数(fanout);空闲码为 0。
    pub fn fanout(&self, code: &KeySequence) -> usize {
        self.group(code).map_or(0, |group| group.len())
    }

    /// 碰撞类型分类。
    pub fn collision_class(&self, code: &KeySequence) -> CollisionClass {
        let Some(group) = self.group(code) else {
            return CollisionClass::None;
        };
        let mut sources: Vec<CandidateSource> = group.iter().map(|c| c.source).collect();
        sources.sort_unstable();
        sources.dedup();
        if sources.len() > 1 {
            return CollisionClass::Multiple;
        }
        match (sources[0], code.len()) {
            (CandidateSource::Level1Shortcut, _) => CollisionClass::Level1,
            (CandidateSource::CharCode, 2) => CollisionClass::Char2Key,
            (CandidateSource::CharCode, 3) => CollisionClass::Char3Key,
            (CandidateSource::CharCode, _) => CollisionClass::FullCodeChar,
            (CandidateSource::FixedWord, _) => CollisionClass::FixedWord,
            (CandidateSource::WordShortcut, _) => CollisionClass::WordShortcut,
        }
    }

    /// 全部已占用码(字典序升序),供统计遍历。
    pub fn occupied_codes(&self) -> impl Iterator<Item = &KeySequence> {
        self.groups.keys()
    }

    /// 按码长的占用统计(1..=8 每层一条,无占用的层 rows 为 0)。
    pub fn length_stats(&self) -> Vec<LengthStats> {
        (1..=8)
            .map(|length| LengthStats::compute(length, self))
            .collect()
    }

    /// 分层行数审计。
    pub fn layer_audit(&self) -> LayerAudit {
        let mut audit = LayerAudit::default();
        for (code, group) in &self.groups {
            for candidate in group {
                match candidate.source {
                    CandidateSource::Level1Shortcut => audit.level1_shortcut_rows += 1,
                    CandidateSource::CharCode => match code.len() {
                        2 => audit.char_2key_rows += 1,
                        3 => audit.char_3key_rows += 1,
                        _ => audit.char_4key_rows += 1,
                    },
                    CandidateSource::FixedWord => match code.len() {
                        4 => audit.word_4key_rows += 1,
                        6 => audit.word_6key_rows += 1,
                        _ => audit.word_8key_rows += 1,
                    },
                    CandidateSource::WordShortcut => match code.len() {
                        3 => audit.word_shortcut_3key_rows += 1,
                        4 => audit.word_shortcut_4key_rows += 1,
                        5 => audit.word_shortcut_5key_rows += 1,
                        6 => audit.word_shortcut_6key_rows += 1,
                        _ => audit.word_shortcut_7key_rows += 1,
                    },
                }
            }
        }
        audit
    }
}

/// 单个码长的占用统计。
pub struct LengthStats {
    length: usize,
    distinct_codes: usize,
    rows: usize,
    mean_fanout: f64,
    median_fanout: usize,
    p90_fanout: usize,
    p95_fanout: usize,
    p99_fanout: usize,
    max_fanout: usize,
}

impl LengthStats {
    fn compute(length: usize, occupancy: &CodeOccupancy) -> Self {
        let mut fanouts: Vec<usize> = occupancy
            .groups
            .keys()
            .filter(|code| code.len() == length)
            .map(|code| occupancy.fanout(code))
            .collect();
        fanouts.sort_unstable();
        let rows: usize = fanouts.iter().sum();
        let distinct_codes = fanouts.len();
        let percentile = |p: usize| -> usize {
            if fanouts.is_empty() {
                return 0;
            }
            // 最近秩百分位
            let index = (p * fanouts.len()).div_ceil(100).max(1) - 1;
            fanouts[index]
        };
        LengthStats {
            length,
            distinct_codes,
            rows,
            mean_fanout: if distinct_codes == 0 {
                0.0
            } else {
                rows as f64 / distinct_codes as f64
            },
            median_fanout: percentile(50),
            p90_fanout: percentile(90),
            p95_fanout: percentile(95),
            p99_fanout: percentile(99),
            max_fanout: fanouts.last().copied().unwrap_or(0),
        }
    }

    /// 码长(键数)。
    pub fn length(&self) -> usize {
        self.length
    }

    /// 该码长的 distinct 已占用码数。
    pub fn distinct_codes(&self) -> usize {
        self.distinct_codes
    }

    /// 该码长的条目行数。
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// 平均扇出。
    pub fn mean_fanout(&self) -> f64 {
        self.mean_fanout
    }

    /// 中位扇出(最近秩)。
    pub fn median_fanout(&self) -> usize {
        self.median_fanout
    }

    /// P90 扇出(最近秩)。
    pub fn p90_fanout(&self) -> usize {
        self.p90_fanout
    }

    /// P95 扇出(最近秩)。
    pub fn p95_fanout(&self) -> usize {
        self.p95_fanout
    }

    /// P99 扇出(最近秩)。
    pub fn p99_fanout(&self) -> usize {
        self.p99_fanout
    }

    /// 最大扇出。
    pub fn max_fanout(&self) -> usize {
        self.max_fanout
    }
}

/// 分层行数审计结果。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LayerAudit {
    /// 一级简码行数(1 键)。
    pub level1_shortcut_rows: usize,
    /// 2 码单字行数。
    pub char_2key_rows: usize,
    /// 3 码单字行数。
    pub char_3key_rows: usize,
    /// 4 码单字(规范全码)行数。
    pub char_4key_rows: usize,
    /// 4 键词语行数。
    pub word_4key_rows: usize,
    /// 6 键词语行数。
    pub word_6key_rows: usize,
    /// 8 键词语行数。
    pub word_8key_rows: usize,
    /// 3 键词语简码行数。
    pub word_shortcut_3key_rows: usize,
    /// 4 键词语简码行数。
    pub word_shortcut_4key_rows: usize,
    /// 5 键词语简码行数。
    pub word_shortcut_5key_rows: usize,
    /// 6 键词语简码行数。
    pub word_shortcut_6key_rows: usize,
    /// 7 键词语简码行数。
    pub word_shortcut_7key_rows: usize,
}

impl LayerAudit {
    /// 词语简码层全部行数(3~7 键合计)。
    pub fn word_shortcut_rows(&self) -> usize {
        self.word_shortcut_3key_rows
            + self.word_shortcut_4key_rows
            + self.word_shortcut_5key_rows
            + self.word_shortcut_6key_rows
            + self.word_shortcut_7key_rows
    }

    /// 全部行数合计。
    pub fn total_rows(&self) -> usize {
        self.level1_shortcut_rows
            + self.char_2key_rows
            + self.char_3key_rows
            + self.char_4key_rows
            + self.word_4key_rows
            + self.word_6key_rows
            + self.word_8key_rows
            + self.word_shortcut_rows()
    }
}
