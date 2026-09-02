//! 频率模型:domain 内归一化 + char:word 混合,raw 仅作诊断。
//!
//! 万象词语分数与单字读音分数**不保证处于相同绝对尺度**,因此主模型在两个
//! domain 内分别归一化(P_word / P_char),再通过 char:word 混合比合并;
//! raw-score 模型只作为 sensitivity 诊断,不作为推荐依据。

use std::collections::BTreeMap;

use xhup_generator::{CharCodeAnalysisEntry, WordCodeAnalysisEntry};

/// 3 码单字多码归属假设(同一汉字可能拥有多个合法 3 码形码)。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum CharCodeUsage {
    /// 每条 3 码关系保留完整频率证据(高估扰动,保守上界)。
    Conservative,
    /// 汉字的频率证据在其 distinct 3 码间均分(中性近似)。
    Split,
}

impl CharCodeUsage {
    /// 报告用稳定标签。
    pub fn label(self) -> &'static str {
        match self {
            CharCodeUsage::Conservative => "conservative",
            CharCodeUsage::Split => "split",
        }
    }
}

/// 频率尺度假设。
#[derive(Clone, Copy, Debug)]
pub enum FrequencyScale {
    /// 主模型:domain 内归一化 + char:word 混合。
    Normalized {
        /// 单字 domain 的占比(char:word 混合比中的 char 侧)。
        char_share: f64,
        /// 3 码单字多码归属假设。
        usage: CharCodeUsage,
    },
    /// 诊断:直接混用 raw 分数(词/字不同尺度的风险未处理)。
    RawDiagnostic,
}

impl FrequencyScale {
    /// 报告用稳定标签。
    pub fn label(&self) -> String {
        match *self {
            FrequencyScale::Normalized { char_share, usage } => format!(
                "normalized(char:{:.0}% word:{:.0}% {})",
                char_share * 100.0,
                (1.0 - char_share) * 100.0,
                usage.label()
            ),
            FrequencyScale::RawDiagnostic => "raw-diagnostic".to_string(),
        }
    }
}

/// 频率证据的 domain 归一化状态(构建一次,各 sweep 复用)。
pub struct FrequencyModel {
    word_total: u64,
    char_total_conservative: u64,
    char_total_split: f64,
    /// 每个汉字的 distinct 3 码数(用于 Split 均分)。
    three_key_code_counts: BTreeMap<char, usize>,
}

impl FrequencyModel {
    /// 从分析证据投影构建频率模型。
    pub fn build(chars: &[CharCodeAnalysisEntry], words: &[WordCodeAnalysisEntry]) -> Self {
        let word_total = words.iter().map(|e| e.frequency_score()).sum();
        let char_total_conservative = chars.iter().map(|e| e.frequency_score()).sum();
        let mut three_key_code_counts: BTreeMap<char, usize> = BTreeMap::new();
        for entry in chars {
            if entry.code().len() == 3 {
                *three_key_code_counts
                    .entry(entry.hanzi().as_char())
                    .or_default() += 1;
            }
        }
        let char_total_split = chars
            .iter()
            .map(|entry| {
                let score = entry.frequency_score() as f64;
                if entry.code().len() == 3 {
                    let count = three_key_code_counts
                        .get(&entry.hanzi().as_char())
                        .copied()
                        .unwrap_or(1)
                        .max(1);
                    score / count as f64
                } else {
                    score
                }
            })
            .sum();
        FrequencyModel {
            word_total,
            char_total_conservative,
            char_total_split,
            three_key_code_counts,
        }
    }

    /// 词语 domain 内归一化概率:P_word(w) = score / Σ word scores。
    pub fn word_probability(&self, score: u64) -> f64 {
        score as f64 / self.word_total.max(1) as f64
    }

    /// 单字条目在指定归属假设下的未归一化质量。
    pub fn char_mass(
        &self,
        hanzi: char,
        code_length: usize,
        score: u64,
        usage: CharCodeUsage,
    ) -> f64 {
        match usage {
            CharCodeUsage::Conservative => score as f64,
            CharCodeUsage::Split => {
                if code_length == 3 {
                    let count = self
                        .three_key_code_counts
                        .get(&hanzi)
                        .copied()
                        .unwrap_or(1)
                        .max(1);
                    score as f64 / count as f64
                } else {
                    score as f64
                }
            }
        }
    }

    /// 单字 domain 内归一化概率。
    pub fn char_probability(
        &self,
        hanzi: char,
        code_length: usize,
        score: u64,
        usage: CharCodeUsage,
    ) -> f64 {
        let mass = self.char_mass(hanzi, code_length, score, usage);
        let total = match usage {
            CharCodeUsage::Conservative => self.char_total_conservative as f64,
            CharCodeUsage::Split => self.char_total_split,
        };
        mass / total.max(f64::MIN_POSITIVE)
    }

    /// 分析目标(词语)在指定频率尺度下的权重。
    pub fn target_weight(&self, scale: &FrequencyScale, word_score: u64) -> f64 {
        match *scale {
            FrequencyScale::Normalized { char_share, .. } => {
                (1.0 - char_share) * self.word_probability(word_score)
            }
            FrequencyScale::RawDiagnostic => word_score as f64,
        }
    }

    /// 既有候选在指定频率尺度下的权重(用于 OPTIMIZED 扰动计量)。
    ///
    /// 一级简码无频率证据且不可被扰动(长度 1,shortcut 不可达),恒为 0。
    pub fn candidate_weight(
        &self,
        scale: &FrequencyScale,
        source: crate::occupancy::CandidateSource,
        text: &str,
        code_length: usize,
        score: u64,
    ) -> f64 {
        use crate::occupancy::CandidateSource;
        match *scale {
            FrequencyScale::Normalized { char_share, usage } => match source {
                CandidateSource::Level1Shortcut => 0.0,
                CandidateSource::CharCode => {
                    let hanzi = text.chars().next().expect("单字候选恰为一字");
                    char_share * self.char_probability(hanzi, code_length, score, usage)
                }
                CandidateSource::FixedWord => (1.0 - char_share) * self.word_probability(score),
            },
            FrequencyScale::RawDiagnostic => score as f64,
        }
    }
}
