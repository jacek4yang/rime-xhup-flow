//! 分析证据投影:面向 `xhup-analyzer` 的窄只读视图。
//!
//! 本模块只把最终化条目集中分析所需的四类证据(文本、码、万象聚合频率
//! 分数、显式 Rime 权重)投影为稳定的公共类型;最终化内部类型
//! (`FinalizedCharCodeEntry` / `FinalizedWordCodeEntry`)与排名实现保持
//! crate 私有。本模块不产生任何新数据,也不参与 production 生成:
//! 所有 Rime/Trainer 产物字节与是否存本投影无关。

use xhup_core::{KeySequence, XhupHanzi};

use crate::char_codes::finalized_char_code_entries;
use crate::word_codes::finalized_word_code_entries;

/// 分析证据投影:一条静态单字编码关系(2/3/4 码)。
///
/// 与 [`crate::canonical_char_code_entries`] 共享同一份最终化条目集,
/// 额外携带万象聚合频率分数,供编码空间/频率收益分析使用。
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct CharCodeAnalysisEntry {
    hanzi: XhupHanzi,
    code: KeySequence,
    frequency_score: u64,
    rime_weight: u32,
}

impl CharCodeAnalysisEntry {
    /// 该条目对应的规范汉字。
    pub fn hanzi(&self) -> XhupHanzi {
        self.hanzi
    }

    /// 该条目对应的静态输入码(2/3/4 键)。
    pub fn code(&self) -> &KeySequence {
        &self.code
    }

    /// 贡献读音的万象聚合分数(u64,可为 0 = 无频率证据)。
    ///
    /// 与单字 domain 内其他条目可比;不保证与词语分数处于相同绝对尺度。
    pub fn frequency_score(&self) -> u64 {
        self.frequency_score
    }

    /// 显式 Rime 权重:同码候选中正数且唯一,越大排名越靠前。
    pub fn rime_weight(&self) -> u32 {
        self.rime_weight
    }
}

/// 全部静态单字编码关系的分析证据投影。
///
/// 返回顺序与最终化条目集的序列化顺序一致
/// (码长升序、码字典序升序、权重降序、汉字 Unicode 标量升序)。
pub fn char_code_analysis_entries() -> Vec<CharCodeAnalysisEntry> {
    finalized_char_code_entries()
        .iter()
        .map(|entry| CharCodeAnalysisEntry {
            hanzi: entry.hanzi(),
            code: entry.code().clone(),
            frequency_score: entry.frequency_score(),
            rime_weight: entry.rime_weight(),
        })
        .collect()
}

/// 分析证据投影:一条静态词语编码关系(4/6/8 键)。
///
/// 与 [`crate::canonical_word_code_entries`] 共享同一份最终化条目集,
/// 额外携带万象聚合频率分数,供编码空间/频率收益分析使用。
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct WordCodeAnalysisEntry {
    word: String,
    code: KeySequence,
    frequency_score: u64,
    rime_weight: u32,
}

impl WordCodeAnalysisEntry {
    /// 该条目对应的词语。
    pub fn word(&self) -> &str {
        &self.word
    }

    /// 该条目对应的静态输入码(4/6/8 键,逐字双拼两键按字序拼接)。
    pub fn code(&self) -> &KeySequence {
        &self.code
    }

    /// 唯一贡献读音序列的万象聚合分数(u64)。
    ///
    /// 与词语 domain 内其他条目可比;不保证与单字分数处于相同绝对尺度。
    pub fn frequency_score(&self) -> u64 {
        self.frequency_score
    }

    /// 显式 Rime 权重:同码候选中正数且唯一,越大排名越靠前。
    pub fn rime_weight(&self) -> u32 {
        self.rime_weight
    }
}

/// 全部静态词语编码关系的分析证据投影。
///
/// 返回顺序与最终化条目集的序列化顺序一致
/// (码长升序、码字典序升序、权重降序、词 Unicode 标量升序)。
pub fn word_code_analysis_entries() -> Vec<WordCodeAnalysisEntry> {
    finalized_word_code_entries()
        .iter()
        .map(|entry| WordCodeAnalysisEntry {
            word: entry.word().to_string(),
            code: entry.code().clone(),
            frequency_score: entry.frequency_score(),
            rime_weight: entry.rime_weight(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{canonical_char_code_entries, canonical_word_code_entries};

    #[test]
    fn char_projection_matches_public_production_projection() {
        let analysis = char_code_analysis_entries();
        let production = canonical_char_code_entries();
        assert_eq!(analysis.len(), production.len());
        for (a, p) in analysis.iter().zip(production.iter()) {
            assert_eq!(a.hanzi(), p.hanzi());
            assert_eq!(a.code(), p.code());
            assert_eq!(a.rime_weight(), p.weight());
        }
    }

    #[test]
    fn word_projection_matches_public_production_projection() {
        let analysis = word_code_analysis_entries();
        let production = canonical_word_code_entries();
        assert_eq!(analysis.len(), production.len());
        for (a, p) in analysis.iter().zip(production.iter()) {
            assert_eq!(a.word(), p.word());
            assert_eq!(a.code(), p.code());
            assert_eq!(a.rime_weight(), p.weight());
        }
    }

    #[test]
    fn projection_carries_real_frequency_evidence() {
        // 已知高分样本:「的」(单字)与「我们」(词语)应有非零万象分数
        let chars = char_code_analysis_entries();
        let de = chars
            .iter()
            .find(|e| e.hanzi().as_char() == '的' && e.code().to_string() == "de")
            .expect("的 de 应存在");
        assert!(de.frequency_score() > 0);
        let words = word_code_analysis_entries();
        let women = words
            .iter()
            .find(|e| e.word() == "我们")
            .expect("我们应存在");
        assert!(women.frequency_score() > 0);
    }
}
