//! 本地用户自适应加分的解码侧消费层(Issue #83 §25 第 9 步 / §16)。
//!
//! 加分语义与封顶全部由 `xhup-analyzer` 的 `UserModel` 定义并序列化
//! (`xhup-user-model/v1`);本模块只做**消费**:把「词 → 有界加分」查表
//! 叠加到 baseline 主项上,不重复实现学习逻辑,也不持久化任何数据。
//!
//! # 为什么加分必须有界且永不为负
//!
//! §6 的核心教训是「弱证据必须平滑降级」:本地学习是**个人 overlay**,
//! 绝不能盖过数量级的静态证据差,更不能压制任何候选(否则降低 OOV/
//! open-composition 可达性,违反 §24)。因此:
//!
//! - 加分 ≥ 0,由上游 [`crate::scoring::log2_q10`] 同源的 Q10 定点表达;
//! - 无本地信号的词得 0 分 —— 得分与 baseline **严格一致**,与
//!   [`KdconvBigramScorer`] 一样提供干净的 A/B 对照;
//! - 查表经 `BTreeMap`/`BTreeSet` 语义,评分不依赖 HashMap 顺序或浮点,
//!   相同状态必然得到相同分数(§24 确定性红线)。
//!
//! 上游导入/导出、损坏回退、重置语义见 `xhup-analyzer::user_model`;
//! 运行时(如 Lua 侧)持有本地状态并通过版本化快照喂给本层,快照本身
//! **不进入普通日志**(用户词即用户文本,§5 隐私红线)。

use crate::scoring::{BaselineScoreBreakdown, BaselineScorer, DeterministicScorer, Score};
use crate::{Lattice, LatticePath, RuntimeContext};

/// scorer 标识(解释与报告用)。
pub const USER_BOOST_SCORER_ID: &str = "user-boost/v1";

/// 用户加分查表。
///
/// 刻意以 `&dyn Fn` 形态接入,避免把 `xhup-analyzer` 的具体模型类型拖进
/// 本 crate 的依赖面:调用方(评测管线 / 未来 Lua 桥)可用真实模型、
/// 截断后的快照或测试 stub 实现同一接口。
pub trait UserBoostLookup {
    /// 返回该词的本地加分(Q10,非负);未知词返回 0。
    fn boost_q10(&self, word: &str, now_seq: u64) -> Score;
}

/// `Fn` 适配器:让闭包与 `UserModel` 引用都能直接当查表用。
impl<F> UserBoostLookup for F
where
    F: Fn(&str, u64) -> Score,
{
    fn boost_q10(&self, word: &str, now_seq: u64) -> Score {
        self(word, now_seq)
    }
}

/// baseline + 本地用户加分的 scorer(`user-boost/v1`)。
///
/// 得分 = [`BaselineScorer`] 总分 + Σ 路径各段的用户加分。路径内重复词形
/// 按段次计数(加分语义按「每次出现都消费一次信号」解释,与词频主项的
/// 逐段累加同构);加分上界由上游模型保证,本层只透传非负值。
#[derive(Clone, Debug)]
pub struct UserBoostScorer<L> {
    baseline: BaselineScorer,
    lookup: L,
}

impl<L> UserBoostScorer<L>
where
    L: UserBoostLookup,
{
    pub const SCORER_ID: &'static str = USER_BOOST_SCORER_ID;

    /// 用给定查表构造;baseline 取默认(与 A/B 对照口径一致)。
    pub fn new(lookup: L) -> Self {
        Self {
            baseline: BaselineScorer::default(),
            lookup,
        }
    }

    /// 显式指定 baseline(敏感度分析用)。
    pub fn with_baseline(mut self, baseline: BaselineScorer) -> Self {
        self.baseline = baseline;
        self
    }
}

/// user-boost scorer 的可解释分数组成。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserBoostBreakdown {
    /// baseline 部分总分(词频奖励 − 分段惩罚)。
    pub baseline: BaselineScoreBreakdown,
    /// 本地加分合计(0 = 本词无用户信号,得分与 baseline 一致)。
    pub user_boost: Score,
}

impl<L> DeterministicScorer for UserBoostScorer<L>
where
    L: UserBoostLookup,
{
    type Breakdown = UserBoostBreakdown;

    fn score(
        &self,
        context: &RuntimeContext,
        lattice: &Lattice,
        path: &LatticePath,
    ) -> (Score, Self::Breakdown) {
        let (baseline_score, baseline_breakdown) = self.baseline.score(context, lattice, path);
        let now_seq = context.committed_left_chars() as u64;
        let user_boost = path.edge_ids().iter().fold(0_i64, |sum, &id| {
            let edge = lattice.edge(id).expect("path edge 必须属于 lattice");
            let boost = self.lookup.boost_q10(edge.candidate().text(), now_seq);
            sum.saturating_add(boost.max(0))
        });
        (
            baseline_score.saturating_add(user_boost),
            UserBoostBreakdown {
                baseline: baseline_breakdown,
                user_boost,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CandidateKind, EdgeCandidate, Span};
    use std::num::NonZeroUsize;
    use xhup_core::KeySequence;

    const LIMIT: NonZeroUsize = NonZeroUsize::new(64).expect("64 != 0");

    fn lattice_with(code: &str, words: &[(&str, u64)]) -> Lattice {
        let key_seq: KeySequence = code.parse().expect("合法码");
        let len = key_seq.len();
        let mut lattice = Lattice::new(key_seq);
        let span = Span::new(0, len).expect("码长 ≥ 1");
        for (word, frequency) in words {
            lattice
                .add_edge(
                    span,
                    EdgeCandidate::new((*word).to_string(), CandidateKind::HotWord, *frequency)
                        .expect("候选文本非空"),
                )
                .expect("同码候选共享同一合法 span");
        }
        lattice
    }

    #[test]
    fn no_user_signal_scores_identically_to_baseline() {
        let lattice = lattice_with("uij", &[("时间", 500), ("已见", 100)]);
        let context = RuntimeContext::new("", "uij".parse().unwrap());
        let scorer = UserBoostScorer::new(|_word: &str, _now: u64| 0);
        let paths = lattice.complete_paths(LIMIT);
        let ranked = crate::rank_paths(&scorer, &context, &lattice, paths.paths());
        let baseline_ranked = crate::rank_paths(
            &BaselineScorer::default(),
            &context,
            &lattice,
            paths.paths(),
        );
        let texts: Vec<String> = ranked.iter().map(|p| p.text().to_string()).collect();
        let baseline_texts: Vec<String> = baseline_ranked
            .iter()
            .map(|p| p.text().to_string())
            .collect();
        assert_eq!(texts, baseline_texts, "零加分必须逐路径与 baseline 一致");
    }

    #[test]
    fn user_boost_can_lift_learned_word_but_respects_static_mass() {
        // 词频差 500 vs 100 的 log2_q10 奖励差约 2.4 千:单次封顶加分
        // (2048)翻不了 —— 这正是「本地信号是 overlay、不是一键置顶」的
        // 量化体现。差距缩到 500 vs 300(奖励差约 1.4 千)时,封顶加分
        // 足以翻转**接近**的同码对;数量级的差距则永远翻不动(下例)。
        let lattice = lattice_with("uij", &[("时间", 500), ("已见", 300)]);
        let context = RuntimeContext::new("", "uij".parse().unwrap());
        let scorer = UserBoostScorer::new(
            |word: &str, _now: u64| {
                if word == "已见" { 2048 } else { 0 }
            },
        );
        let paths = lattice.complete_paths(LIMIT);
        let ranked = crate::rank_paths(&scorer, &context, &lattice, paths.paths());
        assert_eq!(
            ranked.first().expect("有路径").text(),
            "已见",
            "封顶加分应能翻转接近的同码对"
        );

        let big = lattice_with("uij", &[("时间", 1_000_000), ("已见", 100)]);
        let context = RuntimeContext::new("", "uij".parse().unwrap());
        let paths = big.complete_paths(LIMIT);
        let ranked = crate::rank_paths(&scorer, &context, &big, paths.paths());
        assert_eq!(
            ranked.first().expect("有路径").text(),
            "时间",
            "本地加分不得压过数量级的静态证据差"
        );
    }

    #[test]
    fn breakdown_reports_baseline_and_boost_parts() {
        let lattice = lattice_with("uij", &[("时间", 500)]);
        let context = RuntimeContext::new("", "uij".parse().unwrap());
        let scorer = UserBoostScorer::new(
            |word: &str, _now: u64| {
                if word == "时间" { 1024 } else { 0 }
            },
        );
        let paths = lattice.complete_paths(LIMIT);
        let (score, breakdown) =
            scorer.score(&context, &lattice, paths.paths().first().expect("有路径"));
        assert_eq!(breakdown.user_boost, 1024);
        assert_eq!(score, breakdown.baseline.total + breakdown.user_boost);
    }

    #[test]
    fn scoring_is_deterministic_across_calls() {
        let lattice = lattice_with("uij", &[("时间", 500), ("已见", 100)]);
        let context = RuntimeContext::new("", "uij".parse().unwrap());
        let scorer = UserBoostScorer::new(
            |word: &str, now: u64| {
                if word == "已见" && now == 3 { 512 } else { 0 }
            },
        );
        let paths = lattice.complete_paths(LIMIT);
        let a = crate::rank_paths(&scorer, &context, &lattice, paths.paths());
        let b = crate::rank_paths(&scorer, &context, &lattice, paths.paths());
        assert_eq!(
            a.iter()
                .map(|p| (p.text().to_string(), p.score()))
                .collect::<Vec<_>>(),
            b.iter()
                .map(|p| (p.text().to_string(), p.score()))
                .collect::<Vec<_>>(),
            "同状态必须同分(确定性)"
        );
    }
}
