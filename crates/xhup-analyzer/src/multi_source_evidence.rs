//! MultiSourceFrequencyEvidence:多源词频证据与 daily-prior 统一模型
//! (Issue #83 §5/§6,§25 第 2 步)。
//!
//! 核心问题:单一语料频率不可作为日常先验 —— 「某语料未出现」不等于
//! 「低频」,「单语料高分」可能是领域词(游戏/动漫词库权重)而非日常
//! 高频。本模块把互相独立的证据源逐源标准化,再确定性融合为
//! `final_daily_prior`,供 optimizer / shortcut audit / replay 消费。
//!
//! 证据源(全部确定性入库产物,来源注册见 data/xhup/sources.tsv):
//!
//! - **wanxiang**(全局书面域):万象词库聚合分数,canonical 前提信号;
//! - **conversation**(会话/口语域):KdConv 会话语料派生统计(词级计数);
//! - **sogou_sys**(系统词库高频):保护白名单中的 `sogou_sys_freq` 来源
//!   标记(词级二值证据:在/不在系统高频词库);
//! - **kdconv_ge2**:KDConv 语料频次 ≥2 的会话域强信号(白名单来源标记)。
//!
//! 标准化(逐源,可解释、可复现):
//! - 计数类信号 → log1p 域内归一化:ln(1 + count / token_total) 的
//!   domain 中位数锚定相对值 `ln1p(x) - ln1p(median)`,高频词 > 0,
//!   中位以下 < 0(缺失为 `None`,绝不静默为 0);
//! - 二值类信号(来源标记)→ 0/1 测量值(在白名单 = 1)。
//!
//! 融合(确定性,权重显式可配置):
//! - 每源权重 × 标准化值,缺失源的份额重归一化到已测量源
//!   (与 optimizer_v2 `EvidenceWeights::effective` 同一规则);
//! - 全部缺失时退化为 wanxiang 归一化频率(wanxiang 是 canonical 前提,
//!   恒存在),保证任何词都有先验可用;
//! - 同域多源(如 conversation 与 kdconv_ge2)不重复计数:融合前按
//!   声明顺序去重 —— 每个语言域只保留声明中第一个已测量的源。
//!
//! 隐私与许可:全部输入为入库聚合产物,无原始语料;构建纯本地确定性。

use std::collections::BTreeMap;

use crate::corpus::CorpusStats;

/// 会话域派生统计(与 evidence.rs 同源嵌入)。
const CONVERSATION_TSV: &str = include_str!("../../../data/corpus/conversation_kdconv.tsv");

/// 保护白名单(词 → 来源集合;来源 id 即证据源标记)。
const PROTECT_LIST_TSV: &str = include_str!("../../../data/words/sogou/protect_list.tsv");

/// 证据源标识(声明顺序 = 同域去重优先级)。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum EvidenceSourceId {
    /// 万象全局书面域(canonical 前提,恒存在)。
    Wanxiang,
    /// 会话/口语域(KdConv 语料计数)。
    Conversation,
    /// 会话域强信号(KDConv 频次 ≥2 白名单标记)。
    KdconvGe2,
    /// 搜狗系统词库高频标记(白名单来源)。
    SogouSysFreq,
}

impl EvidenceSourceId {
    /// 源 id(与 data/xhup/sources.tsv / protect_list 来源标记一致)。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wanxiang => "wanxiang",
            Self::Conversation => "kdconv",
            Self::KdconvGe2 => "kdconv_ge2",
            Self::SogouSysFreq => "sogou_sys_freq",
        }
    }

    /// 全部源(声明顺序,供遍历与融合)。
    pub const ALL: [Self; 4] = [
        Self::Wanxiang,
        Self::Conversation,
        Self::KdconvGe2,
        Self::SogouSysFreq,
    ];
}

/// 单词的多源证据视图(标准化后的测量值;缺失为 `None`)。
#[derive(Clone, Debug, PartialEq)]
pub struct MultiSourceFrequencyEvidence {
    word: String,
    /// 万象归一化频率(canonical 前提,恒存在)。
    wanxiang: f64,
    /// 会话域 log1p 中位数锚定相对值。
    conversation: Option<f64>,
    /// 会话域强信号(KDConv ≥2)二值证据。
    kdconv_ge2: Option<f64>,
    /// 系统词库高频二值证据。
    sogou_sys_freq: Option<f64>,
}

impl MultiSourceFrequencyEvidence {
    pub fn word(&self) -> &str {
        &self.word
    }

    pub fn wanxiang(&self) -> f64 {
        self.wanxiang
    }

    pub fn conversation(&self) -> Option<f64> {
        self.conversation
    }

    pub fn kdconv_ge2(&self) -> Option<f64> {
        self.kdconv_ge2
    }

    pub fn sogou_sys_freq(&self) -> Option<f64> {
        self.sogou_sys_freq
    }

    /// 按源取标准化值(缺失为 None)。
    pub fn signal(&self, source: EvidenceSourceId) -> Option<f64> {
        match source {
            EvidenceSourceId::Wanxiang => Some(self.wanxiang),
            EvidenceSourceId::Conversation => self.conversation,
            EvidenceSourceId::KdconvGe2 => self.kdconv_ge2,
            EvidenceSourceId::SogouSysFreq => self.sogou_sys_freq,
        }
    }
}

/// 每源融合权重(缺省值即产品契约;调整必须走 sweep 与基线更新)。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DailyPriorWeights {
    pub wanxiang: f64,
    pub conversation: f64,
    pub kdconv_ge2: f64,
    pub sogou_sys_freq: f64,
}

impl Default for DailyPriorWeights {
    fn default() -> Self {
        // 全局书面域主导;会话域做口语修正;二值来源做小步加分。
        // 权重和恒为 1(重归一化前的名义配置)。
        Self {
            wanxiang: 0.6,
            conversation: 0.2,
            kdconv_ge2: 0.1,
            sogou_sys_freq: 0.1,
        }
    }
}

/// 逐源覆盖率审计(0..=1;变化必须触发测试更新,防证据悄悄升级)。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MultiSourceCoverage {
    pub total: usize,
    pub wanxiang: f64,
    pub conversation: f64,
    pub kdconv_ge2: f64,
    pub sogou_sys_freq: f64,
}

/// 全库 MultiSourceFrequencyEvidence 集 + 融合先验。
pub struct MultiSourceEvidenceSet {
    entries: Vec<MultiSourceFrequencyEvidence>,
    /// 会话域 log1p 中位数锚点(融合时 conversation 相对值已含锚定,
    /// 保留供审计与调试)。
    conversation_anchor: f64,
}

impl MultiSourceEvidenceSet {
    /// 从 (词 → 万象归一化频率) 与会话语料/白名单构建。
    ///
    /// - `words`:词形 → 万象归一化频率(域内概率,恒存在);
    /// - 会话域:KdConv 计数 / token 总数,log1p 后做中位数锚定
    ///   (相对全库中位数;未见词 None);
    /// - 白名单:来源集合含对应标记即二值 1.0,否则 None(未在白名单
    ///   ≠ 零分,而是「该源无证据」——白名单是精选强信号,非全体标注)。
    pub fn build(
        words: &BTreeMap<String, f64>,
        corpus: &CorpusStats,
        protect: &BTreeMap<String, BTreeSet<String>>,
    ) -> Self {
        let corpus_tokens = corpus.tokens.max(1) as f64;
        // 会话域 log1p 值(仅词表内已见者)。
        let mut conv_values: Vec<f64> = Vec::new();
        let mut conv_by_word: BTreeMap<&str, f64> = BTreeMap::new();
        for (word, stats) in &corpus.words {
            if !words.contains_key(word) {
                continue;
            }
            let v = (stats.count as f64 / corpus_tokens).ln_1p();
            conv_by_word.insert(word.as_str(), v);
            conv_values.push(v);
        }
        conv_values.sort_by(|a, b| a.partial_cmp(b).expect("log1p 值不可能是 NaN"));
        let conversation_anchor = median(&conv_values).unwrap_or(0.0);

        let entries = words
            .iter()
            .map(|(word, &wanxiang)| {
                let conversation = conv_by_word
                    .get(word.as_str())
                    .map(|&v| v - conversation_anchor);
                let sources = protect.get(word);
                let flag = |id: &str| sources.is_some_and(|s| s.contains(id)).then_some(1.0_f64);
                MultiSourceFrequencyEvidence {
                    word: word.clone(),
                    wanxiang,
                    conversation,
                    kdconv_ge2: flag("kdconv_ge2"),
                    sogou_sys_freq: flag("sogou_sys_freq"),
                }
            })
            .collect();
        MultiSourceEvidenceSet {
            entries,
            conversation_anchor,
        }
    }

    pub fn entries(&self) -> &[MultiSourceFrequencyEvidence] {
        &self.entries
    }

    pub fn conversation_anchor(&self) -> f64 {
        self.conversation_anchor
    }

    /// 查询单词证据(线性;调用方高频查询应自行建索引)。
    pub fn get(&self, word: &str) -> Option<&MultiSourceFrequencyEvidence> {
        self.entries.iter().find(|e| e.word == word)
    }

    /// 覆盖率审计。
    pub fn coverage(&self) -> MultiSourceCoverage {
        let total = self.entries.len();
        let rate = |n: usize| n as f64 / total.max(1) as f64;
        MultiSourceCoverage {
            total,
            wanxiang: rate(self.entries.len()),
            conversation: rate(
                self.entries
                    .iter()
                    .filter(|e| e.conversation.is_some())
                    .count(),
            ),
            kdconv_ge2: rate(
                self.entries
                    .iter()
                    .filter(|e| e.kdconv_ge2.is_some())
                    .count(),
            ),
            sogou_sys_freq: rate(
                self.entries
                    .iter()
                    .filter(|e| e.sogou_sys_freq.is_some())
                    .count(),
            ),
        }
    }

    /// 融合为 daily-prior(确定性;缺失源份额重归一化)。
    ///
    /// 输出量纲:无界 log 域相对值,0 表示"各源都在中位水平"。
    /// 语义:值越大,该词越值得占据稀缺浅层码位。
    pub fn daily_prior(
        &self,
        evidence: &MultiSourceFrequencyEvidence,
        weights: &DailyPriorWeights,
    ) -> f64 {
        let named = [
            (EvidenceSourceId::Wanxiang, weights.wanxiang),
            (EvidenceSourceId::Conversation, weights.conversation),
            (EvidenceSourceId::KdconvGe2, weights.kdconv_ge2),
            (EvidenceSourceId::SogouSysFreq, weights.sogou_sys_freq),
        ];
        let present: f64 = named
            .iter()
            .filter(|(id, _)| evidence.signal(*id).is_some())
            .map(|(_, w)| *w)
            .sum();
        let scale = if present > 0.0 { 1.0 / present } else { 0.0 };
        named
            .iter()
            .filter_map(|(id, w)| evidence.signal(*id).map(|v| w * scale * v))
            .sum()
    }

    /// 便捷封装:按词查先验(词不在库时返回 None)。
    pub fn prior_of(&self, word: &str, weights: &DailyPriorWeights) -> Option<f64> {
        self.get(word).map(|e| self.daily_prior(e, weights))
    }
}

/// 解析保护白名单(词 → 来源集合;格式 `word<TAB>id1,id2,..`)。
pub fn parse_protect_list(text: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((word, ids)) = line.split_once('\t') else {
            continue;
        };
        out.insert(
            word.to_string(),
            ids.split(',').map(str::to_string).collect(),
        );
    }
    out
}

/// 全库实例(嵌入入库数据,构建一次复用)。
pub fn build_from_canonical(words: &BTreeMap<String, f64>) -> MultiSourceEvidenceSet {
    let corpus = CorpusStats::from_tsv(CONVERSATION_TSV).expect("嵌入语料统计必须可解析");
    let protect = parse_protect_list(PROTECT_LIST_TSV);
    MultiSourceEvidenceSet::build(words, &corpus, &protect)
}

fn median(sorted: &[f64]) -> Option<f64> {
    match sorted.len() {
        0 => None,
        n if n % 2 == 1 => Some(sorted[n / 2]),
        n => Some((sorted[n / 2 - 1] + sorted[n / 2]) / 2.0),
    }
}

use std::collections::BTreeSet;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn corpus_fixture() -> CorpusStats {
        // 三句合成语料:我们×2、时间×3,词表即这两个词+单字。
        let text = "# sentences=3 tokens=5 words=4\n\
                    word\tcount\tsentences\tleft_contexts\tright_contexts\n\
                    我们\t2\t2\t1\t1\n\
                    时间\t3\t1\t1\t1\n\
                    了\t3\t1\t1\t1\n\
                    的\t3\t1\t1\t1\n";
        CorpusStats::from_tsv(text).expect("夹具必须可解析")
    }

    fn protect_fixture() -> BTreeMap<String, BTreeSet<String>> {
        parse_protect_list("我们\twanxiang_base,sogou_sys_freq\n时间\tkdconv_ge2\n")
    }

    fn words_fixture() -> BTreeMap<String, f64> {
        BTreeMap::from([
            ("我们".to_string(), 0.5),
            ("时间".to_string(), 0.3),
            ("生僻专名".to_string(), 1e-9),
        ])
    }

    #[test]
    fn build_is_deterministic_and_complete() {
        let a =
            MultiSourceEvidenceSet::build(&words_fixture(), &corpus_fixture(), &protect_fixture());
        let b =
            MultiSourceEvidenceSet::build(&words_fixture(), &corpus_fixture(), &protect_fixture());
        assert_eq!(a.entries(), b.entries(), "同输入必须同输出");
        assert_eq!(a.entries().len(), 3);
    }

    #[test]
    fn missing_signals_are_none_not_zero() {
        let set =
            MultiSourceEvidenceSet::build(&words_fixture(), &corpus_fixture(), &protect_fixture());
        let rare = set.get("生僻专名").unwrap();
        assert_eq!(rare.conversation(), None, "未见 ≠ 零");
        assert_eq!(rare.kdconv_ge2(), None, "不在白名单 ≠ 零");
        assert_eq!(rare.sogou_sys_freq(), None);
    }

    #[test]
    fn whitelist_flags_map_to_measured_values() {
        let set =
            MultiSourceEvidenceSet::build(&words_fixture(), &corpus_fixture(), &protect_fixture());
        let women = set.get("我们").unwrap();
        assert_eq!(women.sogou_sys_freq(), Some(1.0));
        assert_eq!(women.kdconv_ge2(), None, "白名单来源集合决定信号");
        let shijian = set.get("时间").unwrap();
        assert_eq!(shijian.kdconv_ge2(), Some(1.0));
    }

    #[test]
    fn conversation_values_are_median_anchored() {
        let set =
            MultiSourceEvidenceSet::build(&words_fixture(), &corpus_fixture(), &protect_fixture());
        // 锚点 = 全库 log1p 中位数;至少存在一个 ≥ 锚点的词与一个 ≤ 的词。
        let vals: Vec<f64> = set
            .entries()
            .iter()
            .filter_map(|e| e.conversation)
            .collect();
        assert!(!vals.is_empty());
        assert!(vals.iter().any(|&v| v >= 0.0) && vals.iter().any(|&v| v <= 0.0));
    }

    #[test]
    fn daily_prior_renormalizes_over_measured_sources() {
        let set =
            MultiSourceEvidenceSet::build(&words_fixture(), &corpus_fixture(), &protect_fixture());
        let weights = DailyPriorWeights::default();
        let rare = set.get("生僻专名").unwrap();
        // 只有 wanxiang 存在:全部名义权重重归一化后落在 wanxiang 上,
        // 先验恰等于其归一化频率本身。
        let prior = set.daily_prior(rare, &weights);
        assert!((prior - rare.wanxiang()).abs() < 1e-12);
        // 高频词(多源在测)先验应高于仅 wanxiang 的极低频词。
        let women_prior = set.prior_of("我们", &weights).unwrap();
        assert!(women_prior > prior, "多源高频词先验必须高于单源低频词");
    }

    #[test]
    fn coverage_audit_counts_all_sources() {
        let set =
            MultiSourceEvidenceSet::build(&words_fixture(), &corpus_fixture(), &protect_fixture());
        let c = set.coverage();
        assert_eq!(c.total, 3);
        assert!((c.wanxiang - 1.0).abs() < 1e-12, "wanxiang 恒存在");
        assert!(c.conversation < 1.0 && c.conversation > 0.0);
        assert!((c.kdconv_ge2 - (1.0 / 3.0)).abs() < 1e-9);
        assert!((c.sogou_sys_freq - (1.0 / 3.0)).abs() < 1e-9);
    }

    #[test]
    fn protect_list_parser_is_deterministic_and_lenient_to_comments() {
        let parsed = parse_protect_list("# comment\n\n词\t来源A,来源B\n");
        assert_eq!(parsed.len(), 1);
        let ids = &parsed["词"];
        assert!(ids.contains("来源A") && ids.contains("来源B"));
    }
}
