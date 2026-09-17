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

#[cfg(test)]
mod tests {
    use super::explain_production_word;

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
