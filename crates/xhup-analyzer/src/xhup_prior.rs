//! XHUP 风格先验(docs/optimizer-v2.md §3):传统/官方小鹤映射是可学习的
//! 设计先验,不是不可变真理,也不是无关遗留。
//!
//! 参考映射由使用方**本地提供、绝不入库**(官方派生数据许可红线,见
//! docs/data-pipeline.md);本模块只定义先验的数学形式与计算,测试全部
//! 使用合成参考数据(不触碰任何真实官方映射)。
//!
//! 先验得分语义(0..=1):
//!
//! - 1.0:与传统映射完全一致(同词同码);
//! - 0.7:同码但不同候选位(传统把它放次选,我们放首选,或反之);
//! - 0.3:同词不同码(传统用别的码);
//! - 0.5:参考映射未覆盖该词(中立,不奖不惩);
//!
//! 优化器消费方式:`xhup_deviation_coeff × (1 - prior)` 是偏离代价;
//! prior 强度本身是扫描参数 —— 优化器**可以**在真实证据显著更优时
//! 偏离传统,但偏离被记录进兼容率对照报告供评审(compat 模块)。

use crate::compat::{ReferenceEntry, parse_reference_tsv};

/// XHUP 风格先验:参考映射的索引视图。
pub struct XhupStylePrior {
    /// 词 → 参考 (码, 排名) 列表(一词多码按排名升序)。
    by_text: std::collections::BTreeMap<String, Vec<(String, u32)>>,
    /// (词, 码) → 参考排名。
    by_pair: std::collections::BTreeMap<(String, String), u32>,
}

/// 参考映射缺失时的中立得分(未覆盖不奖不惩)。
pub const NEUTRAL_PRIOR: f64 = 0.5;

impl XhupStylePrior {
    /// 从参考 TSV 文本构建(格式见 compat::parse_reference_tsv)。
    pub fn from_tsv(text: &str) -> Result<Self, String> {
        let entries = parse_reference_tsv(text)?;
        Ok(Self::from_entries(entries))
    }

    /// 从解析后的参考条目构建。
    pub fn from_entries(entries: Vec<ReferenceEntry>) -> Self {
        let mut by_text: std::collections::BTreeMap<String, Vec<(String, u32)>> =
            std::collections::BTreeMap::new();
        let mut by_pair = std::collections::BTreeMap::new();
        for entry in entries {
            by_text
                .entry(entry.text.clone())
                .or_default()
                .push((entry.code.clone(), entry.rank));
            by_pair.insert((entry.text, entry.code), entry.rank);
        }
        for list in by_text.values_mut() {
            list.sort_by_key(|(_, rank)| *rank);
        }
        XhupStylePrior { by_text, by_pair }
    }

    /// 空先验(无参考数据):全部中立。
    pub fn empty() -> Self {
        XhupStylePrior {
            by_text: Default::default(),
            by_pair: Default::default(),
        }
    }

    /// 词 W 在码 C 候选位 R 的先验得分(0..=1)。
    pub fn score(&self, word: &str, code: &str, _rank: usize) -> f64 {
        match self.by_pair.get(&(word.to_string(), code.to_string())) {
            Some(&1) => 1.0, // 完全一致(传统首选位)
            Some(_) => 0.7,  // 同码不同位
            None => match self.by_text.get(word) {
                Some(_) => 0.3,        // 同词不同码
                None => NEUTRAL_PRIOR, // 未覆盖,中立
            },
        }
    }

    /// 参考映射是否覆盖该词(报告用)。
    pub fn covers(&self, word: &str) -> bool {
        self.by_text.contains_key(word)
    }

    /// 参考条目数。
    pub fn len(&self) -> usize {
        self.by_pair.len()
    }

    /// 是否为空(无参考数据)。
    pub fn is_empty(&self) -> bool {
        self.by_pair.is_empty()
    }
}

#[cfg(test)]
mod tests {
    //! 全部使用合成参考数据(许可红线:真实官方映射不入库、不进测试)。
    use super::*;

    fn sample_prior() -> XhupStylePrior {
        XhupStylePrior::from_tsv(
            "# 合成参考(非真实数据)\n我们\twm\t1\n我们\twomf\t2\n时间\tuij\t1\n",
        )
        .unwrap()
    }

    #[test]
    fn exact_match_scores_one() {
        let prior = sample_prior();
        assert_eq!(prior.score("时间", "uij", 1), 1.0);
    }

    #[test]
    fn same_code_other_rank_scores_partial() {
        let prior = sample_prior();
        assert_eq!(prior.score("我们", "wm", 2), 1.0, "参考首选位");
        // 参考中 (我们, wm) 是 rank 1;我们 在 womf 是 rank 2。
        assert_eq!(prior.score("我们", "womf", 1), 0.7, "同码不同位");
    }

    #[test]
    fn same_word_other_code_scores_low() {
        let prior = sample_prior();
        assert_eq!(prior.score("时间", "uijm", 1), 0.3, "同词不同码");
    }

    #[test]
    fn uncovered_word_is_neutral() {
        let prior = sample_prior();
        assert_eq!(prior.score("数据", "ujjm", 1), 0.5);
        assert!(!prior.covers("数据"));
        assert!(prior.covers("我们"));
    }

    #[test]
    fn empty_prior_is_neutral_everywhere() {
        let prior = XhupStylePrior::empty();
        assert!(prior.is_empty());
        assert_eq!(prior.score("任何", "abcd", 1), NEUTRAL_PRIOR);
    }

    #[test]
    fn invalid_tsv_is_rejected() {
        assert!(XhupStylePrior::from_tsv("我们\tWM\t1\n").is_err());
        assert!(XhupStylePrior::from_tsv("我们\twm\n").is_err());
    }
}
