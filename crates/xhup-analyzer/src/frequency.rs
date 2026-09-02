//! 频率模型:domain 内归一化 + char:word 混合,raw 仅作诊断。
//!
//! 万象词语分数与单字读音分数**不保证处于相同绝对尺度**,因此主模型在两个
//! domain 内分别归一化(P_word / P_char),再通过 char:word 混合比合并;
//! raw-score 模型只作为 sensitivity 诊断,不作为推荐依据。
//!
//! Normalized 模型的单字 domain **只含 3 码单字关系**:OPTIMIZED profile 中
//! 唯一允许与词语 shortcut 自由竞争并可被扰动的既有单字层(4 码规范全码被
//! 硬保护、1/2 键对长度 ≥ 3 的 shortcut 不可达)。把 2/4 码关系计入分母会
//! 稀释 char domain,使 `char:word` 混合比失去意义,因此这里严格排除。

use std::collections::BTreeMap;

use xhup_core::{Key, KeySequence};
use xhup_generator::{CharCodeAnalysisEntry, WordCodeAnalysisEntry};

/// 3 码单字多码归属假设(同一汉字/读音可能拥有多个合法 3 码形码)。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum CharCodeUsage {
    /// 每条 3 码关系保留完整频率证据(高估扰动,保守上界)。
    Conservative,
    /// 频率证据按 (汉字, 声码前缀 = 前两键) 在其 distinct 3 码间均分
    /// (中性近似;不同读音的形码备选互不混入)。
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
    /// 3 码单字关系的 Conservative 频率总和(归一化分母)。
    char_total_conservative: u64,
    /// 3 码单字关系按 (字, 声码前缀) 均分后的 Split 频率总和。
    char_total_split: f64,
    /// (汉字, 声码前两键) → distinct 3 码数(用于 Split 均分)。
    three_key_code_counts: BTreeMap<(char, Key, Key), usize>,
}

impl FrequencyModel {
    /// 从分析证据投影构建频率模型。
    pub fn build(chars: &[CharCodeAnalysisEntry], words: &[WordCodeAnalysisEntry]) -> Self {
        let word_total = words.iter().map(|e| e.frequency_score()).sum();
        // Normalized 单字 domain 仅含 3 码关系。
        let three_key: Vec<&CharCodeAnalysisEntry> =
            chars.iter().filter(|e| e.code().len() == 3).collect();
        let char_total_conservative = three_key.iter().map(|e| e.frequency_score()).sum();
        let mut three_key_code_counts: BTreeMap<(char, Key, Key), usize> = BTreeMap::new();
        for entry in &three_key {
            let keys = entry.code().as_slice();
            *three_key_code_counts
                .entry((entry.hanzi().as_char(), keys[0], keys[1]))
                .or_default() += 1;
        }
        let char_total_split = three_key
            .iter()
            .map(|entry| {
                let keys = entry.code().as_slice();
                let count = three_key_code_counts
                    .get(&(entry.hanzi().as_char(), keys[0], keys[1]))
                    .copied()
                    .expect("不变量:3 码关系必然已计数");
                entry.frequency_score() as f64 / count as f64
            })
            .sum();
        FrequencyModel {
            word_total,
            char_total_conservative,
            char_total_split,
            three_key_code_counts,
        }
    }

    /// 3 码单字关系 Conservative 未归一化频率总和(审计用)。
    pub fn char_total_conservative(&self) -> u64 {
        self.char_total_conservative
    }

    /// 3 码单字关系 Split 未归一化频率总和(审计用)。
    pub fn char_total_split(&self) -> f64 {
        self.char_total_split
    }

    /// 词语 domain 内归一化概率:P_word(w) = score / Σ word scores。
    pub fn word_probability(&self, score: u64) -> f64 {
        score as f64 / self.word_total.max(1) as f64
    }

    /// 3 码单字关系在指定归属假设下的未归一化质量。
    fn three_key_mass(
        &self,
        hanzi: char,
        sound_prefix: (Key, Key),
        score: u64,
        usage: CharCodeUsage,
    ) -> f64 {
        match usage {
            CharCodeUsage::Conservative => score as f64,
            CharCodeUsage::Split => {
                let count = self
                    .three_key_code_counts
                    .get(&(hanzi, sound_prefix.0, sound_prefix.1))
                    .copied()
                    .unwrap_or(1)
                    .max(1);
                score as f64 / count as f64
            }
        }
    }

    /// 3 码单字关系的 domain 内归一化概率。
    fn three_key_probability(
        &self,
        hanzi: char,
        sound_prefix: (Key, Key),
        score: u64,
        usage: CharCodeUsage,
    ) -> f64 {
        let mass = self.three_key_mass(hanzi, sound_prefix, score, usage);
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
    /// Normalized 模型下单字扰动权重只对 3 码关系定义(唯一可重排的单字层);
    /// 对其它码长请求单字权重违反不变量,直接 panic。
    pub fn candidate_weight(
        &self,
        scale: &FrequencyScale,
        source: crate::occupancy::CandidateSource,
        text: &str,
        code: &KeySequence,
        score: u64,
    ) -> f64 {
        use crate::occupancy::CandidateSource;
        match *scale {
            FrequencyScale::Normalized { char_share, usage } => match source {
                CandidateSource::Level1Shortcut => 0.0,
                CandidateSource::CharCode => {
                    assert_eq!(
                        code.len(),
                        3,
                        "不变量:Normalized 单字扰动权重只对 3 码关系定义\
                         (4 码规范全码硬保护,1/2 键 shortcut 不可达)"
                    );
                    let keys = code.as_slice();
                    let hanzi = text.chars().next().expect("单字候选恰为一字");
                    char_share * self.three_key_probability(hanzi, (keys[0], keys[1]), score, usage)
                }
                CandidateSource::FixedWord => (1.0 - char_share) * self.word_probability(score),
            },
            FrequencyScale::RawDiagnostic => score as f64,
        }
    }
}

#[cfg(test)]
mod tests {
    //! 归一化不变量使用真实 canonical 数据验证:单字 domain 的归一化质量和
    //! 必须恒为 1,混合后的 char 侧总质量必须等于设定的 char_share。
    use super::*;
    use crate::AnalysisData;
    use crate::occupancy::CandidateSource;

    /// 遍历全部 3 码单字关系,求指定尺度下的 candidate_weight 总和。
    fn char_mass_total(data: &AnalysisData, scale: &FrequencyScale) -> f64 {
        let mut buf = [0u8; 4];
        data.chars
            .iter()
            .filter(|e| e.code().len() == 3)
            .map(|e| {
                data.frequency.candidate_weight(
                    scale,
                    CandidateSource::CharCode,
                    e.hanzi().as_char().encode_utf8(&mut buf),
                    e.code(),
                    e.frequency_score(),
                )
            })
            .sum()
    }

    #[test]
    fn normalized_char_domain_sums_to_one() {
        let data = crate::build_analysis();
        for usage in [CharCodeUsage::Conservative, CharCodeUsage::Split] {
            let total = char_mass_total(
                &data,
                &FrequencyScale::Normalized {
                    char_share: 1.0,
                    usage,
                },
            );
            assert!(
                (total - 1.0).abs() < 1e-9,
                "{}:归一化 3 码单字质量和应≈1.0, 实际 {total}",
                usage.label()
            );
        }
    }

    #[test]
    fn mixture_preserves_char_share() {
        let data = crate::build_analysis();
        for usage in [CharCodeUsage::Conservative, CharCodeUsage::Split] {
            for char_share in [0.25, 0.50, 0.75] {
                let total =
                    char_mass_total(&data, &FrequencyScale::Normalized { char_share, usage });
                assert!(
                    (total - char_share).abs() < 1e-9,
                    "{} char_share={char_share}:char 侧总质量应≈{char_share}, 实际 {total}",
                    usage.label()
                );
            }
        }
    }
}
