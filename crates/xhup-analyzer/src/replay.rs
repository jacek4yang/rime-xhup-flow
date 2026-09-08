//! 语料回放基准(Input Model v2,docs/optimizer-v2.md §5 的度量底座)。
//!
//! 给定句子语料与当前 canonical 映射,模拟「最佳静态码」输入,产出
//! 确定性指标:KSPC(键/字)、候选选择分布(rank1/rank≤3 命中率)、
//! 每词输入方案(键数 + 预期 rank + 经由层)。用途:
//!
//! - 优化器工作点比较(A/B:baseline vs 候选映射);
//! - 发布门禁:映射变更必须有回放数据支撑(对照报告见 compat 模块);
//! - 回归守卫:回放指标意外退化必须显式可见。
//!
//! 语义边界:这是**静态层**回放(全码 + 简码别名 + 候选位),不模拟
//! 组句/学习/上下文 —— 那些层的收益由 flow 审计与语言模型基准单独度量。
//! 分词与语料统计同源(canonical 词表最大匹配)。

use std::collections::BTreeMap;

use xhup_generator::{
    canonical_fixed_first_shortcut_entries, canonical_two_key_shortcut_entries,
    canonical_word_code_entries, canonical_word_shortcut_entries,
};

use crate::corpus::Segmenter;

/// 回放成本假设(集中、可配;数值是无量纲优化目标,非真实耗时)。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReplayCostModel {
    /// 每键基础成本。
    pub key_cost: f64,
    /// 候选选择成本(索引 = rank-1;rank ≥ 4 用末元素)。
    pub rank_cost: [f64; 4],
}

impl Default for ReplayCostModel {
    fn default() -> Self {
        ReplayCostModel {
            key_cost: 1.0,
            rank_cost: [0.0, 0.5, 1.0, 2.0],
        }
    }
}

/// 一个词的最佳静态输入方案。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputPlan {
    /// 键数。
    pub keys: usize,
    /// 预期候选位(1 = 首选)。
    pub rank: usize,
    /// 经由层(full / zero-regression / fixed-first / two-key)。
    pub via: &'static str,
    /// 期望成本(键 + 选择)。
    pub expected_cost: f64,
}

/// 回放视图:词 → 最佳静态输入方案(canonical 映射,或经
/// [`ReplayMapping::build_with_plans`] 叠加的任意评估映射)。
#[derive(Clone)]
pub struct ReplayMapping {
    plans: BTreeMap<String, InputPlan>,
}

impl ReplayMapping {
    /// 从 canonical 数据构建(全码 + ZR + FF + 二码,按期望成本择优)。
    pub fn build(cost: &ReplayCostModel) -> Self {
        let mut mapping = Self::build_full_code_layer(cost);

        // 简码层:码位唯一映射,首选。
        for entry in canonical_word_shortcut_entries() {
            mapping.offer(
                cost,
                entry.word(),
                entry.shortcut_code().len(),
                1,
                "zero-regression",
            );
        }
        for entry in canonical_fixed_first_shortcut_entries() {
            mapping.offer(
                cost,
                entry.word(),
                entry.shortcut_code().len(),
                1,
                "fixed-first",
            );
        }
        for entry in canonical_two_key_shortcut_entries() {
            mapping.offer(
                cost,
                entry.word(),
                entry.shortcut_code().len(),
                1,
                "two-key",
            );
        }
        mapping
    }

    /// 从 canonical 全码层 + 调用方提供的词语映射构建(v2 扫描等任意
    /// 映射评估用;docs/optimizer-v2.md §5)。
    ///
    /// 与 [`Self::build`] 的区别:**不含**已入库的 ZR/FF/二码简码层;
    /// `plans` = (词, 键数, 预期候选位),与全码层按期望成本择优(同成本
    /// 先到先得,输入须确定性有序)。
    pub fn build_with_plans(cost: &ReplayCostModel, plans: &[(String, usize, usize)]) -> Self {
        let mut mapping = Self::build_full_code_layer(cost);
        for (word, keys, rank) in plans {
            mapping.offer(cost, word, *keys, *rank, "v2-sweep");
        }
        mapping
    }

    /// 全码层:候选位 = 同码权重降序名次(词词典内;4 键碰撞码的字词
    /// 跨表合并由 merged_ranking 仲裁 —— 词侧相对次序不变,字侧竞争
    /// 已在 common_word_coverage 门禁中单独保证,回放按词词典视图)。
    fn build_full_code_layer(cost: &ReplayCostModel) -> Self {
        let mut mapping = ReplayMapping {
            plans: BTreeMap::new(),
        };
        let word_entries = canonical_word_code_entries();
        let mut by_code: BTreeMap<String, Vec<(u32, &str)>> = BTreeMap::new();
        for entry in &word_entries {
            by_code
                .entry(entry.code().to_string())
                .or_default()
                .push((entry.weight(), entry.word()));
        }
        for (code, mut entries) in by_code {
            entries.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
            for (rank, (_, word)) in entries.iter().enumerate() {
                mapping.offer(cost, word, code.chars().count(), rank + 1, "full");
            }
        }
        mapping
    }

    /// 提供一条方案:期望成本更低则替换现有方案。
    fn offer(
        &mut self,
        cost: &ReplayCostModel,
        word: &str,
        keys: usize,
        rank: usize,
        via: &'static str,
    ) {
        let selection_cost =
            |rank: usize| cost.rank_cost[rank.saturating_sub(1).min(cost.rank_cost.len() - 1)];
        let plan = InputPlan {
            keys,
            rank,
            via,
            expected_cost: keys as f64 * cost.key_cost + selection_cost(rank),
        };
        let better = match self.plans.get(word) {
            None => true,
            Some(existing) => plan.expected_cost < existing.expected_cost,
        };
        if better {
            self.plans.insert(word.to_string(), plan);
        }
    }

    /// 词的输入方案(未收录词 = None,调用方按逐字全码兜底)。
    pub fn plan(&self, word: &str) -> Option<InputPlan> {
        self.plans.get(word).copied()
    }
}

/// 单句回放结果。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SentenceReplay {
    /// 汉字数(分词 token 覆盖的字符)。
    pub chars: usize,
    /// 分词 token 数。
    pub tokens: usize,
    /// 键数合计。
    pub keys: usize,
    /// 首选命中 token 数。
    pub rank1: usize,
    /// 前三命中 token 数。
    pub top3: usize,
    /// 期望成本合计。
    pub expected_cost: f64,
    /// 未收录 token 数(逐字全码兜底 = 字长 × 4 键,rank 1)。
    pub fallback_tokens: usize,
}

/// 语料回放汇总(确定性)。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReplayReport {
    /// 句子数。
    pub sentences: u64,
    /// 指标合计。
    pub totals: SentenceReplay,
}

impl ReplayReport {
    /// KSPC:键数 / 汉字数。
    pub fn kspc(&self) -> f64 {
        self.totals.keys as f64 / (self.totals.chars.max(1)) as f64
    }

    /// rank-1 命中率(token 级)。
    pub fn rank1_rate(&self) -> f64 {
        self.totals.rank1 as f64 / (self.totals.tokens.max(1)) as f64
    }

    /// rank ≤ 3 命中率(token 级)。
    pub fn top3_rate(&self) -> f64 {
        self.totals.top3 as f64 / (self.totals.tokens.max(1)) as f64
    }
}

/// 回放器:分词 + 逐 token 方案求和。
pub struct Replayer {
    segmenter: Segmenter,
    mapping: ReplayMapping,
}

impl Replayer {
    /// 以当前 canonical 映射构建。
    pub fn new(cost: &ReplayCostModel) -> Self {
        Replayer {
            segmenter: Segmenter::build(),
            mapping: ReplayMapping::build(cost),
        }
    }

    /// 以调用方提供的回放视图构建(v2 扫描等任意映射评估;
    /// 分词仍与 canonical 词表同源)。
    pub fn with_mapping(mapping: ReplayMapping) -> Self {
        Replayer {
            segmenter: Segmenter::build(),
            mapping,
        }
    }

    /// 回放一个句子。
    pub fn replay_sentence(&self, sentence: &str) -> SentenceReplay {
        let mut result = SentenceReplay::default();
        for token in self.segmenter.segment(sentence) {
            let token_chars = token.chars().count();
            result.chars += token_chars;
            result.tokens += 1;
            match self.mapping.plan(&token) {
                Some(plan) => {
                    result.keys += plan.keys;
                    result.expected_cost += plan.expected_cost;
                    if plan.rank == 1 {
                        result.rank1 += 1;
                    }
                    if plan.rank <= 3 {
                        result.top3 += 1;
                    }
                }
                None => {
                    // 逐字全码兜底:每字 4 键,首选(无频率证据不假设竞争)。
                    result.keys += token_chars * 4;
                    result.expected_cost += token_chars as f64 * 4.0;
                    result.rank1 += 1;
                    result.top3 += 1;
                    result.fallback_tokens += 1;
                }
            }
        }
        result
    }

    /// 回放语料(每行一句)。
    pub fn replay_corpus<'a>(&self, sentences: impl Iterator<Item = &'a str>) -> ReplayReport {
        let mut report = ReplayReport::default();
        for sentence in sentences {
            report.sentences += 1;
            let r = self.replay_sentence(sentence);
            report.totals.chars += r.chars;
            report.totals.tokens += r.tokens;
            report.totals.keys += r.keys;
            report.totals.rank1 += r.rank1;
            report.totals.top3 += r.top3;
            report.totals.expected_cost += r.expected_cost;
            report.totals.fallback_tokens += r.fallback_tokens;
        }
        report
    }

    /// 回放加权词形流:聚合统计近似(无句子上下文,每个词形视作独立
    /// 一句,按计数加权)。用于原始句子不可入库时从派生统计(CorpusStats)
    /// 估计回放指标;指标语义与句子回放略有差异(无跨词分词交互),
    /// 报告时必须标注为近似。
    pub fn replay_weighted_words<'a>(
        &self,
        words: impl Iterator<Item = (&'a str, u64)>,
    ) -> ReplayReport {
        let mut report = ReplayReport::default();
        for (word, count) in words {
            let r = self.replay_sentence(word);
            report.sentences += count;
            report.totals.chars += r.chars * count as usize;
            report.totals.tokens += r.tokens * count as usize;
            report.totals.keys += r.keys * count as usize;
            report.totals.rank1 += r.rank1 * count as usize;
            report.totals.top3 += r.top3 * count as usize;
            report.totals.expected_cost += r.expected_cost * count as f64;
            report.totals.fallback_tokens += r.fallback_tokens * count as usize;
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_word_prefers_shortest_path() {
        // 「时间」:全码 uijm 4 键 vs FIXED_FIRST 简码 uij 3 键 → 选 3 键首选
        // (时间 的简码在 FF 层而非 ZR 层,由 canonical 数据锚定)。
        let mapping = ReplayMapping::build(&ReplayCostModel::default());
        let plan = mapping.plan("时间").expect("时间 应有方案");
        assert_eq!(plan.keys, 3);
        assert_eq!(plan.rank, 1);
        assert_eq!(plan.via, "fixed-first");
    }

    #[test]
    fn every_top100_word_has_a_plan() {
        let mapping = ReplayMapping::build(&ReplayCostModel::default());
        let mut entries = xhup_generator::word_code_analysis_entries();
        entries.sort_by_key(|b| std::cmp::Reverse(b.frequency_score()));
        for entry in entries.iter().take(100) {
            assert!(
                mapping.plan(entry.word()).is_some(),
                "{} 应有输入方案",
                entry.word()
            );
        }
    }

    #[test]
    fn replay_is_deterministic_and_counts() {
        let replayer = Replayer::new(&ReplayCostModel::default());
        let a = replayer.replay_sentence("我们时间");
        let b = replayer.replay_sentence("我们时间");
        assert_eq!(a, b);
        assert_eq!(a.chars, 4);
        assert!(a.keys <= 8, "两词全码 8 键是上界,实际 {}", a.keys);
        assert!(a.rank1 >= 1);
    }

    #[test]
    fn fallback_for_unknown_tokens_is_full_code_per_char() {
        let replayer = Replayer::new(&ReplayCostModel::default());
        // 「𬺰𬺱」类生僻组合:词表无该词 → 逐字全码(每字 4 键)。
        let r = replayer.replay_sentence("𫓧𬇙");
        assert!(r.fallback_tokens >= 1 || r.keys >= r.chars * 3);
    }

    #[test]
    fn corpus_replay_aggregates() {
        let replayer = Replayer::new(&ReplayCostModel::default());
        let report = replayer.replay_corpus(["我们时间", "我们"].into_iter());
        assert_eq!(report.sentences, 2);
        assert!(report.kspc() > 0.0 && report.kspc() <= 4.0);
        assert!(report.rank1_rate() > 0.0);
    }

    #[test]
    fn build_with_plans_overlays_arbitrary_mapping() {
        // v2 扫描路径:任意映射与全码层按期望成本择优。
        let cost = ReplayCostModel::default();
        let mapping = ReplayMapping::build_with_plans(&cost, &[("我们".to_string(), 2, 1)]);
        let plan = mapping.plan("我们").expect("我们 应有方案");
        assert_eq!(plan.keys, 2);
        assert_eq!(plan.rank, 1);
        assert_eq!(plan.via, "v2-sweep");
        // 未覆盖的词仍走全码层。
        let full = mapping.plan("时间").expect("时间 应有全码方案");
        assert_eq!(full.via, "full");
    }

    #[test]
    fn weighted_words_replay_scales_by_count() {
        // 聚合统计近似回放 = 单词回放 × 计数。
        let replayer = Replayer::new(&ReplayCostModel::default());
        let single = replayer.replay_corpus(["我们"].into_iter());
        let weighted = replayer.replay_weighted_words([("我们", 3)].into_iter());
        assert_eq!(weighted.sentences, 3);
        assert_eq!(weighted.totals.keys, single.totals.keys * 3);
        assert_eq!(weighted.totals.chars, single.totals.chars * 3);
        assert_eq!(weighted.totals.rank1, single.totals.rank1 * 3);
    }
}
