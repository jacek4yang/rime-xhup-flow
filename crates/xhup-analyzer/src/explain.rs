//! Production mapping v2 单词语决策解释(Trainer / sweep-v2 共用)。
//!
//! 与 `sweep-v2 --explain` 缺省路径同输入:v2_targets 并集、默认
//! CostModelV2 / EvidenceWeights、canonical 传统保底与 v2 先验。
//! 分析输入与候选宇宙按进程 OnceLock 构建一次;无网络。查询词是
//! 调用方显式传入的诊断输入,本模块不写日志。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use xhup_core::KeySequence;

use crate::candidates::WordTarget;
use crate::evidence::{LexicalEvidence, LexicalEvidenceSet};
use crate::mapping_v2::{BaselineMassView, MassScale, produce_mapping_explained, render_explain};
use crate::optimizer_v2::{CostModelV2, EvidenceWeights};
use crate::xhup_prior::XhupStylePrior;

/// sweep-v2 缺省 explain 的不可变输入(进程内构建一次)。
struct ProductionExplain {
    targets: Vec<WordTarget>,
    evidence: BTreeMap<String, LexicalEvidence>,
    scale: MassScale,
    baseline: BaselineMassView,
    prior: XhupStylePrior,
    tradition: BTreeMap<String, KeySequence>,
    /// 有证据且有合法 shortcut 候选的词;不在此集合则无解释卡。
    explainable: BTreeSet<String>,
}

fn production_explain() -> &'static ProductionExplain {
    static CELL: OnceLock<ProductionExplain> = OnceLock::new();
    CELL.get_or_init(|| {
        let data = crate::build_analysis();
        let targets = crate::sweep_v2::v2_targets(&data.words);
        let evidence_set = LexicalEvidenceSet::build(&data.words, &data.frequency);
        let evidence = crate::sweep_v2::evidence_by_word(&evidence_set);
        let scale = MassScale::build(&evidence);
        let baseline = BaselineMassView::build(&data.occupancy);
        let prior = crate::sweep_v2::build_v2_prior(None);
        let tradition = crate::sweep_v2::tradition_map();
        let explainable = targets
            .iter()
            .filter(|target| {
                evidence.contains_key(target.word()) && !target.candidates().is_empty()
            })
            .map(|target| target.word().to_string())
            .collect();
        ProductionExplain {
            targets,
            evidence,
            scale,
            baseline,
            prior,
            tradition,
            explainable,
        }
    })
}

/// 对单个词渲染 mapping v2 ASCII 理由卡。
///
/// 空串/纯空白、或不在 v2 候选宇宙(无证据或无合法候选)时返回
/// `None`。有卡时文本含该词与「全码」列,与 `render_explain` 一致。
pub fn explain_production_word(word: &str) -> Option<String> {
    let word = word.trim();
    if word.is_empty() {
        return None;
    }
    let ctx = production_explain();
    if !ctx.explainable.contains(word) {
        return None;
    }
    let (_, reports) = produce_mapping_explained(
        &ctx.targets,
        &ctx.evidence,
        &ctx.scale,
        &ctx.baseline,
        &CostModelV2::default(),
        &EvidenceWeights::default(),
        Some(&ctx.prior),
        &ctx.tradition,
        &[word],
    );
    reports.get(word).map(render_explain)
}

/// 单词批量解释的逐词结果。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WordExplanation {
    /// 规范化后的查询词(trim;空串拒绝发生在批量入口,不落在此结构)。
    pub word: String,
    /// mapping v2 理由卡;`None` = 词不在 v2 候选宇宙。
    pub mapping_card: Option<String>,
    /// 简码提示卡;`None` = 无提示(候选行不显示 ~<简码>)。
    pub hint_card: Option<String>,
    /// 提示判定的结构化事实(§3 三判:useful/selection/misleading);
    /// None = 无提示。Trainer 过滤(misleading/high-cost/shallow)基于此字段,不解析 ASCII 卡。
    pub hint_verdict: Option<&'static str>,
    /// 提示码省键数(全码长 - 简码长);None = 无提示;0 = 无省键价值。
    pub hint_keys_saved: Option<usize>,
    /// 提示码在真实菜单中的 rank(1 = rank-1 即选);None = 不在该菜单。
    pub hint_menu_rank: Option<usize>,
}

impl WordExplanation {
    /// 该词是否拿到了 mapping v2 解释。
    pub fn has_mapping(&self) -> bool {
        self.mapping_card.is_some()
    }
}

/// 批量解释的聚合读数(供诊断面板头部展示)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct BatchExplanationStats {
    pub total: usize,
    pub with_mapping: usize,
    pub with_hint: usize,
}

/// 批量解释的输入上限(桌面诊断一次渲染的合理边界)。
pub const MAX_EXPLAIN_BATCH: usize = 200;

/// 对一批词(去重、去空、上限 [`MAX_EXPLAIN_BATCH`])批量渲染解释卡。
///
/// 批量契约:
/// - **逐词隔离**:单词不在候选宇宙/无提示只影响该词条目,绝不中断
///   整批(诊断面板一次查几十个词,一个词查不到不应清空全部结果);
/// - **有界**:去重后超过 [`MAX_EXPLAIN_BATCH`] 的输入整体拒绝(调用方
///   分页),防止无界渲染卡死桌面进程;
/// - 输入规范化:逐词 trim,空词丢弃;重复词只解释一次,按首次出现
///   顺序返回(确定性,不依赖输入哈希)。
///
/// 分析输入仍走进程 OnceLock,批量只摊薄一次构建成本 —— 这是批量的
/// 主要性能收益(50 个词与 1 个词的构建成本相同)。
pub fn explain_words_batch(
    words: &[String],
) -> Result<(Vec<WordExplanation>, BatchExplanationStats), String> {
    let mut seen = BTreeSet::new();
    let mut ordered: Vec<&str> = Vec::new();
    for word in words {
        let word = word.trim();
        if word.is_empty() || !seen.insert(word.to_string()) {
            continue;
        }
        ordered.push(word);
    }
    if ordered.len() > MAX_EXPLAIN_BATCH {
        return Err(format!(
            "批量解释一次最多 {} 个词(去重后 {}),请分页查询",
            MAX_EXPLAIN_BATCH,
            ordered.len()
        ));
    }
    let ctx = production_explain();
    let explainable: Vec<&str> = ordered
        .iter()
        .copied()
        .filter(|word| ctx.explainable.contains(*word))
        .collect();
    let reports: BTreeMap<String, crate::mapping_v2::ExplainReport> = if explainable.is_empty() {
        BTreeMap::new()
    } else {
        let (_, reports) = produce_mapping_explained(
            &ctx.targets,
            &ctx.evidence,
            &ctx.scale,
            &ctx.baseline,
            &CostModelV2::default(),
            &EvidenceWeights::default(),
            Some(&ctx.prior),
            &ctx.tradition,
            &explainable,
        );
        reports
    };
    let mut out = Vec::with_capacity(ordered.len());
    let mut with_mapping = 0usize;
    let mut with_hint = 0usize;
    for word in ordered {
        let mapping_card = reports.get(word).map(render_explain);
        let hint = crate::shortcut_explain::explain_shortcut_hint(word);
        let hint_card = hint.as_ref().map(|h| h.render_card());
        if mapping_card.is_some() {
            with_mapping += 1;
        }
        if hint_card.is_some() {
            with_hint += 1;
        }
        out.push(WordExplanation {
            word: word.to_string(),
            mapping_card,
            hint_verdict: hint.as_ref().map(|h| h.verdict_label_static()),
            hint_keys_saved: hint.as_ref().map(|h| h.keystrokes_saved()),
            hint_menu_rank: hint.as_ref().and_then(|h| h.menu_rank),
            hint_card,
        });
    }
    let stats = BatchExplanationStats {
        total: out.len(),
        with_mapping,
        with_hint,
    };
    Ok((out, stats))
}

#[cfg(test)]
mod tests {
    use super::explain_production_word;
    use super::{MAX_EXPLAIN_BATCH, explain_words_batch};
    use std::string::String;

    #[test]
    fn batch_is_deterministic_and_per_word_isolated() {
        let words: Vec<String> = ["我们", "时间", "xyznotaword", "", "  ", "我们"]
            .iter()
            .map(|w| (*w).to_string())
            .collect();
        let (items, stats) = explain_words_batch(&words).expect("合法批量");
        // 去重 + 去空:3 个唯一词(我们、时间、xyznotaword)。
        assert_eq!(stats.total, 3);
        assert_eq!(items.len(), 3);
        // 首次出现顺序(确定性)。
        assert_eq!(items[0].word, "我们");
        assert_eq!(items[1].word, "时间");
        assert_eq!(items[2].word, "xyznotaword");
        // 逐词隔离:未知词只影响自己,不中断整批。
        assert!(!items[2].has_mapping(), "未知词无 mapping 卡");
        assert!(items[0].has_mapping(), "高频词必有 mapping 卡");
        assert_eq!(
            stats.with_mapping,
            items.iter().filter(|i| i.has_mapping()).count()
        );
        // 再次调用逐项一致(确定性,不依赖输入哈希)。
        let (items2, stats2) = explain_words_batch(&words).expect("第二次批量");
        assert_eq!(items, items2);
        assert_eq!(stats, stats2);
    }

    #[test]
    fn batch_rejects_unbounded_input() {
        let words: Vec<String> = (0..(MAX_EXPLAIN_BATCH + 1))
            .map(|i| format!("词{i}"))
            .collect();
        let error = explain_words_batch(&words).expect_err("超限必须整体拒绝");
        assert!(error.contains("分页"), "错误消息应指引分页:{error}");
    }

    #[test]
    fn batch_single_word_matches_single_api() {
        let words = vec!["我们".to_string()];
        let (items, _) = explain_words_batch(&words).expect("单元素批量");
        assert_eq!(items.len(), 1);
        let single = explain_production_word("我们").expect("单查有卡");
        assert_eq!(
            items[0].mapping_card.as_deref(),
            Some(single.as_str()),
            "批量与单查的卡必须逐字节一致(同一 OnceLock 上下文)"
        );
    }

    #[test]
    fn high_frequency_word_yields_ascii_card() {
        let card = explain_production_word("我们").expect("高频词必须有解释卡");
        assert!(!card.is_empty(), "解释卡非空");
        assert!(card.contains("我们"), "解释卡包含查询词");
        assert!(card.contains("全码"), "解释卡包含全码列");
    }

    #[test]
    fn empty_or_unknown_token_returns_none() {
        assert_eq!(explain_production_word(""), None);
        assert_eq!(explain_production_word("   "), None);
        assert_eq!(explain_production_word("xyznotaword"), None);
    }
}
