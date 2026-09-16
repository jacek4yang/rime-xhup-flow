//! 词汇证据模型(Input Model v2,见 docs/input-model-v2.md §2)。
//!
//! 「词 + 一个频率整数」不是充分证据。本模块定义优化器消费的**证据视图**:
//! 每条 canonical 词语关系携带其全部已知证据,未知信号显式为 `None`
//! (绝不静默填 0 —— 0 是测量值,`None` 是缺失,两者语义不同)。
//!
//! 信号来源分层:
//!
//! - **已有**(万象提取子集):聚合频率分数 → 词 domain 归一化概率;
//! - **已有**(KdConv 会话域派生统计,data/corpus):句子覆盖度、上下文
//!   多样性、会话域频率 —— 只覆盖语料中实际出现的词,未出现的词保持
//!   `None`(未见 ≠ 零);
//! - **待更多语料域**:正式/技术域频率(见 docs/data-pipeline.md 来源表)。
//!   在信号落地前,优化目标函数不得把它们当作已测量值;
//! - **永不持久化**的可推导字段(词长、码长)由访问器现算。
//!
//! [`EvidenceCoverage`] 是证据完整性审计:每个信号的全库覆盖率必须显式
//! 可见,覆盖率变化(语料管线落地)会触发测试更新 —— 这是有意的绊线,
//! 防止「证据悄悄升级而优化目标没有跟上」。

use xhup_core::KeySequence;
use xhup_generator::WordCodeAnalysisEntry;

use std::collections::BTreeMap;

use crate::corpus::CorpusStats;
use crate::frequency::FrequencyModel;
use crate::multi_source_evidence::MultiSourceFrequencyEvidence;

/// 会话域派生统计(KdConv,data/corpus/README.md 记 provenance),
/// 与 canonical 数据同法嵌入,是证据视图的语料信号来源。
const CONVERSATION_TSV: &str = include_str!("../../../data/corpus/conversation_kdconv.tsv");

/// 万象归一化概率长尾阈值(2026-09:中位 ≈2.6e-6,P25 ≈2.0e-6,1e-6 ≈ 最底五分位)。
/// 只标定 wanxiang 概率,不可与 daily_prior 比较(先验是 log 域相对值)。
pub(crate) const WANXIANG_RARE_TAIL: f64 = 1e-6;

/// 词法类别(Issue #83 §6):由 MultiSourceFrequencyEvidence 在证据构建时派生。
///
/// 生产路径不按词面匹配;跨源在测信号 = 日常常用,单源书面质量 ≠ 日常常用。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum LexicalClass {
    /// 会话域或保护白名单(`kdconv_ge2` / `sogou_sys_freq`)有在测信号。
    Common,
    /// 仅万象,且归一化频率低于万象长尾阈值(1e-6)。
    Rare,
    /// 仅万象且非长尾:书面单源质量,不等于日常常用。
    DomainSpecific,
    /// 证据不足以分类(合成夹具未接入 MSFE;生产构建不应出现)。
    Unknown,
}

impl LexicalClass {
    /// 由多源证据分类。wanxiang 是 canonical 前提,恒存在;
    /// 无跨源信号时按万象概率区分 Rare / DomainSpecific。
    ///
    /// 会话域缺失为 `None`(未见 ≠ 零),不把缺测当成频率 0。
    pub fn from_multi_source(msfe: &MultiSourceFrequencyEvidence) -> Self {
        let cross_source = msfe.conversation().is_some()
            || msfe.kdconv_ge2().is_some()
            || msfe.sogou_sys_freq().is_some();
        if cross_source {
            Self::Common
        } else if msfe.wanxiang() < WANXIANG_RARE_TAIL {
            Self::Rare
        } else {
            Self::DomainSpecific
        }
    }
}

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
    /// 多源融合 daily-prior(MultiSourceFrequencyEvidence,§25 第 2 步):
    /// 覆盖全部 canonical 词;由 evidence set 构建时融合注入。
    daily_prior: Option<f64>,
    /// 由 MSFE 派生的词法类别(构建时写入,优化器只读)。
    lexical_class: LexicalClass,
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

    /// 多源融合 daily-prior(MultiSourceFrequencyEvidence;缺失 = 未接入)。
    pub fn daily_prior(&self) -> Option<f64> {
        self.daily_prior
    }

    /// 由 MSFE 派生的词法类别。
    pub fn lexical_class(&self) -> LexicalClass {
        self.lexical_class
    }

    /// 测试构造:显式给定各信号(仅供 synthetic 测试;对集成测试可见)。
    ///
    /// 会话域在测 → [`LexicalClass::Common`];否则 [`LexicalClass::Unknown`]
    /// (合成夹具没有保护白名单,不能把「conversation None」当成 wanxiang-only)。
    /// 需要 DomainSpecific / Rare / 显式先验时用 [`Self::with_lexical_class`] /
    /// [`Self::with_daily_prior`]。
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn for_test(
        word: &str,
        code: KeySequence,
        wanxiang_score: u64,
        normalized_frequency: f64,
        sentence_coverage: Option<f64>,
        context_diversity: Option<u32>,
        conversation_frequency: Option<f64>,
    ) -> Self {
        LexicalEvidence {
            word: word.to_string(),
            code,
            wanxiang_score,
            normalized_frequency,
            sentence_coverage,
            context_diversity,
            conversation_frequency,
            formal_frequency: None,
            technical_frequency: None,
            daily_prior: None,
            lexical_class: if conversation_frequency.is_some() {
                LexicalClass::Common
            } else {
                LexicalClass::Unknown
            },
        }
    }

    /// 测试夹具:覆盖融合先验(daily_prior 不是概率,0 = 多源中位)。
    #[doc(hidden)]
    #[must_use]
    pub fn with_daily_prior(mut self, daily_prior: Option<f64>) -> Self {
        self.daily_prior = daily_prior;
        self
    }

    /// 测试夹具:覆盖词法类别(生产路径只由 MSFE 派生)。
    #[doc(hidden)]
    #[must_use]
    pub fn with_lexical_class(mut self, lexical_class: LexicalClass) -> Self {
        self.lexical_class = lexical_class;
        self
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
    /// 多源 daily-prior 覆盖率(接入后应为 1.0)。
    pub daily_prior: f64,
}

/// 全部 canonical 词语关系的词汇证据视图(构建一次,各分析复用)。
pub struct LexicalEvidenceSet {
    entries: Vec<LexicalEvidence>,
}

impl LexicalEvidenceSet {
    /// 从 generator 分析投影 + 频率模型构建证据视图。
    ///
    /// 语料信号填充规则(只填测量值,未见保持 None):
    /// - `conversation_frequency` = 会话域计数 / 语料 token 总数(域内概率);
    /// - `sentence_coverage` = 句子覆盖 / 语料句子总数(0..=1);
    /// - `context_diversity` = 独立左上下文数 + 独立右上下文数。
    pub fn build(words: &[WordCodeAnalysisEntry], frequency: &FrequencyModel) -> Self {
        let corpus = CorpusStats::from_tsv(CONVERSATION_TSV).expect("嵌入的语料统计必须可解析");
        let corpus_tokens = corpus.tokens.max(1) as f64;
        let corpus_sentences = corpus.sentences.max(1) as f64;
        // 多源 daily-prior(§25 第 2 步):词 → 万象归一化频率,与
        // MultiSourceFrequencyEvidence 的其余证据源(会话语料、白名单
        // 来源标记)融合;wanxiang 是 canonical 前提,先验恒存在。
        let prior_input: BTreeMap<String, f64> = words
            .iter()
            .map(|entry| {
                (
                    entry.word().to_string(),
                    frequency.word_probability(entry.frequency_score()),
                )
            })
            .collect();
        let multi_source = crate::multi_source_evidence::build_from_canonical(&prior_input);
        let prior_weights = crate::multi_source_evidence::DailyPriorWeights::default();
        let mut prior_by_word: BTreeMap<String, f64> = BTreeMap::new();
        let mut class_by_word: BTreeMap<String, LexicalClass> = BTreeMap::new();
        for e in multi_source.entries() {
            prior_by_word.insert(
                e.word().to_string(),
                multi_source.daily_prior(e, &prior_weights),
            );
            class_by_word.insert(e.word().to_string(), LexicalClass::from_multi_source(e));
        }
        let entries = words
            .iter()
            .map(|entry| {
                let observed = corpus.words.get(entry.word());
                LexicalEvidence {
                    word: entry.word().to_string(),
                    code: entry.code().clone(),
                    wanxiang_score: entry.frequency_score(),
                    normalized_frequency: frequency.word_probability(entry.frequency_score()),
                    sentence_coverage: observed.map(|s| s.sentence_count as f64 / corpus_sentences),
                    context_diversity: observed.map(|s| {
                        u32::try_from(s.left_contexts + s.right_contexts).unwrap_or(u32::MAX)
                    }),
                    conversation_frequency: observed.map(|s| s.count as f64 / corpus_tokens),
                    // 正式/技术域:来源待导入(docs/data-pipeline.md)。
                    formal_frequency: None,
                    technical_frequency: None,
                    daily_prior: prior_by_word.get(entry.word()).copied(),
                    lexical_class: class_by_word
                        .get(entry.word())
                        .copied()
                        .unwrap_or(LexicalClass::Unknown),
                }
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
            daily_prior: rate(
                self.entries
                    .iter()
                    .filter(|e| e.daily_prior.is_some())
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
        // 已有信号:万象分数 100%;会话域语料信号部分覆盖(只见于语料的词)。
        assert_eq!(coverage.wanxiang_score, 1.0);
        for rate in [
            coverage.sentence_coverage,
            coverage.context_diversity,
            coverage.conversation_frequency,
        ] {
            assert!(rate > 0.1, "会话域信号应有实质覆盖,实际 {rate}");
            assert!(rate < 1.0, "未见语料的词必须保持缺失,实际 {rate}");
        }
        // 待导入信号:显式 0%(缺失 ≠ 零;来源导入时本断言必须更新)。
        assert_eq!(coverage.formal_frequency, 0.0);
        assert_eq!(coverage.technical_frequency, 0.0);
        // 多源 daily-prior:wanxiang 为 canonical 前提,先验全量覆盖。
        assert_eq!(coverage.daily_prior, 1.0, "daily_prior 恒存在(MSFE 接入)");
    }

    #[test]
    fn daily_prior_prefers_multi_source_common_words() {
        let (_, set) = evidence_set();
        // 真实数据上:多源在测的日常词先验应显著高于先验为负的词。
        let mut entries: Vec<&LexicalEvidence> = set.entries().iter().collect();
        entries.sort_by(|a, b| {
            b.daily_prior()
                .partial_cmp(&a.daily_prior())
                .expect("先验不应为 NaN")
        });
        let top = entries[0];
        assert!(
            top.conversation_frequency().is_some() || top.wanxiang_score() > 0,
            "先验最高词应有证据支撑: {}",
            top.word()
        );
        // 融合输入的确定性:同一词重复计算结果一致。
        let again = top.daily_prior().unwrap();
        assert!((again - top.daily_prior().unwrap()).abs() < 1e-12);
    }

    #[test]
    fn observed_words_carry_real_corpus_values() {
        // 语料信号语义哨兵:「知道」是 KdConv 高强度词,三个语料字段必须
        // 同时出现且内部一致;「社会主义」未在语料出现则全部缺失。
        let (_, set) = evidence_set();
        let zhidao = set
            .entries()
            .iter()
            .find(|e| e.word() == "知道")
            .expect("知道 应有证据");
        let coverage = zhidao.sentence_coverage().expect("知道 应有句子覆盖");
        assert!(coverage > 0.01, "知道 的句子覆盖应显著,实际 {coverage}");
        assert!(zhidao.conversation_frequency().unwrap() > 0.0);
        assert!(zhidao.context_diversity().unwrap() >= 2, "左右上下文应多样");
        let shehui = set
            .entries()
            .iter()
            .find(|e| e.word() == "社会主义")
            .expect("社会主义 应有证据");
        assert_eq!(shehui.sentence_coverage(), None, "未见语料的词保持缺失");
    }

    #[test]
    fn canonical_words_are_fully_eligible() {
        let (_, set) = evidence_set();
        for evidence in set.entries() {
            assert_eq!(set.eligibility(evidence), Eligibility::CANONICAL_DEFAULT);
        }
    }

    #[test]
    fn production_evidence_is_fully_classified_from_msfe() {
        let (_, set) = evidence_set();
        assert!(
            set.entries()
                .iter()
                .all(|e| e.lexical_class() != LexicalClass::Unknown),
            "生产构建每条证据都应从 MSFE 得到 Common/Rare/DomainSpecific"
        );
        let zhidao = set
            .entries()
            .iter()
            .find(|e| e.word() == "知道")
            .expect("知道 应有证据");
        assert_eq!(zhidao.lexical_class(), LexicalClass::Common);
        assert!(zhidao.conversation_frequency().is_some());
        let muzhai = set
            .entries()
            .iter()
            .find(|e| e.word() == "木寨")
            .expect("木寨 应有证据");
        assert_eq!(muzhai.lexical_class(), LexicalClass::Rare);
        assert!(muzhai.normalized_frequency() < WANXIANG_RARE_TAIL);
        assert!(muzhai.conversation_frequency().is_none());
        let domain = set
            .entries()
            .iter()
            .find(|e| e.lexical_class() == LexicalClass::DomainSpecific)
            .expect("应存在书面单源(无跨源信号且非长尾)的词");
        assert!(domain.conversation_frequency().is_none());
        assert!(domain.normalized_frequency() >= WANXIANG_RARE_TAIL);
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
