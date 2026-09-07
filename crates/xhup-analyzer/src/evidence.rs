//! 词汇证据模型(Input Model v2,见 docs/input-model-v2.md §2)。
//!
//! 「词 + 一个频率整数」不是充分证据。本模块定义优化器消费的**证据视图**:
//! 每条 canonical 词语关系携带其全部已知证据,未知信号显式为 `None`
//! (绝不静默填 0 —— 0 是测量值,`None` 是缺失,两者语义不同)。
//!
//! 信号来源分层:
//!
//! - **已有**(万象提取子集):聚合频率分数 → 词 domain 归一化概率;
//! - **待语料统计管线**:句子覆盖度、上下文多样性、分域频率(会话/正式/
//!   技术)。在这些信号落地前,优化目标函数不得把它们当作已测量值;
//! - **永不持久化**的可推导字段(词长、码长)由访问器现算。
//!
//! [`EvidenceCoverage`] 是证据完整性审计:每个信号的全库覆盖率必须显式
//! 可见,覆盖率变化(语料管线落地)会触发测试更新 —— 这是有意的绊线,
//! 防止「证据悄悄升级而优化目标没有跟上」。

use xhup_core::KeySequence;
use xhup_generator::WordCodeAnalysisEntry;

use crate::frequency::FrequencyModel;

/// 一条 canonical 词语关系的词汇证据。
#[derive(Clone, Debug, PartialEq)]
pub struct LexicalEvidence {
    /// 词形(canonical,词形唯一)。
    word: String,
    /// 静态完整码(4/6/8 键,逐字双拼两键按字序拼接)。
    code: KeySequence,
    /// 万象聚合频率分数(原始证据,u64,恒正)。
    wanxiang_score: u64,
    /// 词 domain 内归一化概率 P_word(w) = score / Σ word scores。
    normalized_frequency: f64,
    /// 句子覆盖度:该词在真实句子语料中的出现强度(语料管线落地前 None)。
    sentence_coverage: Option<f64>,
    /// 上下文多样性:独立左/右上下文数量(语料管线落地前 None)。
    context_diversity: Option<u32>,
    /// 会话/口语域归一化频率(语料管线落地前 None)。
    conversation_frequency: Option<f64>,
    /// 正式文体域归一化频率(语料管线落地前 None)。
    formal_frequency: Option<f64>,
    /// 技术域归一化频率(语料管线落地前 None)。
    technical_frequency: Option<f64>,
}

impl LexicalEvidence {
    /// 词形。
    pub fn word(&self) -> &str {
        &self.word
    }

    /// 静态完整码。
    pub fn code(&self) -> &KeySequence {
        &self.code
    }

    /// 字数(可推导,不持久化语义)。
    pub fn word_len(&self) -> usize {
        self.word.chars().count()
    }

    /// 万象聚合频率分数(原始证据)。
    pub fn wanxiang_score(&self) -> u64 {
        self.wanxiang_score
    }

    /// 词 domain 内归一化概率。
    pub fn normalized_frequency(&self) -> f64 {
        self.normalized_frequency
    }

    /// 句子覆盖度(缺失 = 语料管线未落地,而非零覆盖)。
    pub fn sentence_coverage(&self) -> Option<f64> {
        self.sentence_coverage
    }

    /// 上下文多样性(缺失 = 语料管线未落地)。
    pub fn context_diversity(&self) -> Option<u32> {
        self.context_diversity
    }

    /// 会话域频率(缺失 = 语料管线未落地)。
    pub fn conversation_frequency(&self) -> Option<f64> {
        self.conversation_frequency
    }

    /// 正式文体域频率(缺失 = 语料管线未落地)。
    pub fn formal_frequency(&self) -> Option<f64> {
        self.formal_frequency
    }

    /// 技术域频率(缺失 = 语料管线未落地)。
    pub fn technical_frequency(&self) -> Option<f64> {
        self.technical_frequency
    }
}

/// 资格分类(Input Model v2 §2.1):四个相互独立的判定,替代
/// 历史「碰撞即删除」的单一生存语义。
///
/// 当前规则(语料管线落地前):所有 canonical 词全部四层 eligible ——
/// 进入 canonical 词库本身即词汇资格;简码稀缺位竞争由优化器的成本
/// 模型约束,不由存在性过滤。语料证据落地后,此处将引入显式的
/// 资格策略(如:长尾专名 shortcut 不可参与、垃圾短语 lexical 拒绝),
/// 每条排除必须有可审计理由。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Eligibility {
    /// 词汇资格:存在于词典(可被全码输入)。
    pub lexical: bool,
    /// 简码资格:允许竞争稀缺简码位(1/2/3 键)。
    pub shortcut: bool,
    /// 组句资格:参与 Flow 语言模型。
    pub flow: bool,
    /// 学习资格:参与本地用户学习。
    pub learning: bool,
}

impl Eligibility {
    /// 当前默认:canonical 词全层 eligible(见类型文档)。
    pub const CANONICAL_DEFAULT: Eligibility = Eligibility {
        lexical: true,
        shortcut: true,
        flow: true,
        learning: true,
    };
}

/// 证据完整性审计:各信号的全库覆盖率(0.0..=1.0)。
///
/// 用途:优化目标函数引用的每个信号必须有显式覆盖率;覆盖率突变的
/// 方向(0 → 1,语料落地;1 → 0,数据源回退)都应触发人工审查。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EvidenceCoverage {
    /// 总条目数。
    pub total: usize,
    /// 万象分数覆盖率(应为 1.0)。
    pub wanxiang_score: f64,
    /// 句子覆盖度覆盖率。
    pub sentence_coverage: f64,
    /// 上下文多样性覆盖率。
    pub context_diversity: f64,
    /// 会话域频率覆盖率。
    pub conversation_frequency: f64,
    /// 正式域频率覆盖率。
    pub formal_frequency: f64,
    /// 技术域频率覆盖率。
    pub technical_frequency: f64,
}

/// 全部 canonical 词语关系的词汇证据视图(构建一次,各分析复用)。
pub struct LexicalEvidenceSet {
    entries: Vec<LexicalEvidence>,
}

impl LexicalEvidenceSet {
    /// 从 generator 分析投影 + 频率模型构建证据视图。
    pub fn build(words: &[WordCodeAnalysisEntry], frequency: &FrequencyModel) -> Self {
        let entries = words
            .iter()
            .map(|entry| LexicalEvidence {
                word: entry.word().to_string(),
                code: entry.code().clone(),
                wanxiang_score: entry.frequency_score(),
                normalized_frequency: frequency.word_probability(entry.frequency_score()),
                // 语料统计管线尚未落地:全部显式缺失(见模块文档)。
                sentence_coverage: None,
                context_diversity: None,
                conversation_frequency: None,
                formal_frequency: None,
                technical_frequency: None,
            })
            .collect();
        LexicalEvidenceSet { entries }
    }

    /// 证据条目(顺序与分析投影一致)。
    pub fn entries(&self) -> &[LexicalEvidence] {
        &self.entries
    }

    /// 证据完整性审计。
    pub fn coverage(&self) -> EvidenceCoverage {
        let total = self.entries.len();
        let rate = |count: usize| count as f64 / total.max(1) as f64;
        EvidenceCoverage {
            total,
            wanxiang_score: rate(self.entries.iter().filter(|e| e.wanxiang_score > 0).count()),
            sentence_coverage: rate(
                self.entries
                    .iter()
                    .filter(|e| e.sentence_coverage.is_some())
                    .count(),
            ),
            context_diversity: rate(
                self.entries
                    .iter()
                    .filter(|e| e.context_diversity.is_some())
                    .count(),
            ),
            conversation_frequency: rate(
                self.entries
                    .iter()
                    .filter(|e| e.conversation_frequency.is_some())
                    .count(),
            ),
            formal_frequency: rate(
                self.entries
                    .iter()
                    .filter(|e| e.formal_frequency.is_some())
                    .count(),
            ),
            technical_frequency: rate(
                self.entries
                    .iter()
                    .filter(|e| e.technical_frequency.is_some())
                    .count(),
            ),
        }
    }

    /// 当前资格分类(全层 eligible,见 [`Eligibility`] 文档)。
    pub fn eligibility(&self, evidence: &LexicalEvidence) -> Eligibility {
        let _ = evidence;
        Eligibility::CANONICAL_DEFAULT
    }
}

#[cfg(test)]
mod tests {
    //! 证据模型不变量用真实 canonical 数据验证。
    use super::*;

    fn evidence_set() -> (crate::AnalysisData, LexicalEvidenceSet) {
        let data = crate::build_analysis();
        let set = LexicalEvidenceSet::build(&data.words, &data.frequency);
        (data, set)
    }

    #[test]
    fn every_canonical_word_has_evidence() {
        let (data, set) = evidence_set();
        assert_eq!(set.entries().len(), data.words.len());
        for (evidence, entry) in set.entries().iter().zip(data.words.iter()) {
            assert_eq!(evidence.word(), entry.word());
            assert_eq!(evidence.code(), entry.code());
            assert_eq!(evidence.wanxiang_score(), entry.frequency_score());
            assert!(evidence.wanxiang_score() > 0, "canonical 词分数恒正");
        }
    }

    #[test]
    fn normalized_frequencies_sum_to_one() {
        let (_, set) = evidence_set();
        let total: f64 = set.entries().iter().map(|e| e.normalized_frequency()).sum();
        assert!(
            (total - 1.0).abs() < 1e-9,
            "词 domain 归一化概率和应≈1.0,实际 {total}"
        );
    }

    #[test]
    fn coverage_audit_reflects_real_signal_availability() {
        let (_, set) = evidence_set();
        let coverage = set.coverage();
        assert_eq!(coverage.total, set.entries().len());
        // 已有信号:万象分数 100%。
        assert_eq!(coverage.wanxiang_score, 1.0);
        // 待语料管线信号:显式 0%(缺失 ≠ 零;语料落地时本断言必须更新)。
        assert_eq!(coverage.sentence_coverage, 0.0);
        assert_eq!(coverage.context_diversity, 0.0);
        assert_eq!(coverage.conversation_frequency, 0.0);
        assert_eq!(coverage.formal_frequency, 0.0);
        assert_eq!(coverage.technical_frequency, 0.0);
    }

    #[test]
    fn canonical_words_are_fully_eligible() {
        let (_, set) = evidence_set();
        for evidence in set.entries() {
            assert_eq!(set.eligibility(evidence), Eligibility::CANONICAL_DEFAULT);
        }
    }

    #[test]
    fn high_frequency_sentinels_carry_stronger_evidence_than_rare_words() {
        // 排序哨兵:骨干口语词的归一化概率必须远高于生僻书面词,
        // 否则证据视图与频率直觉(及 merged_ranking 结论)背离。
        let (_, set) = evidence_set();
        let find = |word: &str| {
            set.entries()
                .iter()
                .find(|e| e.word() == word)
                .unwrap_or_else(|| panic!("{word} 应有证据"))
                .normalized_frequency()
        };
        assert!(find("我们") > find("社会主义") * 10.0);
        assert!(find("可以") > find("众所周知") * 10.0);
    }
}
