//! 跨 scorer 差分:context gain 与 harmful-reorder rate(Issue #83 §20/§24)。
//!
//! 同一 fixture、同一 lattice 分别在两个 scorer 下跑完整 benchmark,逐
//! `case_id` 对齐判定后给出:
//!
//! - **context gain**:baseline 未命中而上下文 scorer 命中的 case 数
//!   (top1 路径正确)。这是「committed context 真的改变了首选路径且改对了」
//!   的可执行证据,而不是叙事。
//! - **harmful reorder rate**:baseline 命中而上下文 scorer 未命中的 case
//!   占比。上下文必须修复歧义,不能以牺牲已经正确的路径为代价;该指标就是
//!   §20 的 `harmful reorder rate`,也是 §6「弱证据确定性平滑降级」的量化门槛。
//!
//! 本模块只做差分与渲染,不改变任何 scorer 语义,不读取用户文本,不联网。

use std::collections::BTreeMap;

use super::{BenchmarkRun, CaseOutcome};

/// 单个 case 在 A/B 两 scorer 下的相对变化。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScoreDelta {
    /// baseline 未命中、上下文 scorer 命中 —— 上下文带来的真实增益。
    Improved,
    /// baseline 命中、上下文 scorer 未命中 —— 有害重排。
    Regressed,
    /// 两者一致(都命中或都未命中)。
    Unchanged,
}

/// 跨 scorer 差分报告(计数 + 逐 case 变化)。
#[derive(Clone, Debug, PartialEq)]
pub struct ScorerComparison {
    /// 参与比较的 baseline scorer 标识。
    pub baseline_scorer: &'static str,
    /// 参与比较的上下文 scorer 标识。
    pub contextual_scorer: &'static str,
    /// baseline 命中(top1 路径正确)的 case 数。
    pub baseline_top1: usize,
    /// 上下文 scorer 命中的 case 数。
    pub contextual_top1: usize,
    /// `Improved` 的 case 数(context gain 的绝对计数)。
    pub improved: usize,
    /// `Regressed` 的 case 数(harmful reorder 的绝对计数)。
    pub regressed: usize,
    /// 两侧都有判定的 case 数(差分分母)。
    pub compared: usize,
    /// 逐 `case_id` 的变化(`BTreeMap` 确定性序)。
    pub deltas: BTreeMap<String, ScoreDelta>,
}

impl ScorerComparison {
    /// context gain:命中率提升(top1 路径正确)。
    pub fn context_gain(&self) -> f64 {
        ratio(self.improved, self.compared)
    }

    /// harmful reorder rate:baseline 正确但被上下文改错的占比。
    pub fn harmful_reorder_rate(&self) -> f64 {
        ratio(self.regressed, self.compared)
    }

    /// net gain:命中率净变化(可为负)。
    pub fn net_gain(&self) -> f64 {
        ratio(self.improved, self.compared) - ratio(self.regressed, self.compared)
    }

    /// 变差的 case 数(harmful reorder 绝对计数)。
    pub fn harmful_cases(&self) -> usize {
        self.regressed
    }

    /// 渲染确定性 ASCII 报告(无 emoji,§7)。
    pub fn render_ascii(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "scorer A/B: {} -> {}\n",
            self.baseline_scorer, self.contextual_scorer
        ));
        out.push_str(&format!("cases compared:        {}\n", self.compared));
        out.push_str(&format!("baseline top1 correct: {}\n", self.baseline_top1));
        out.push_str(&format!(
            "contextual top1 correct: {}\n",
            self.contextual_top1
        ));
        out.push_str(&format!("context gain:          {}\n", self.improved));
        out.push_str(&format!("harmful reorder:       {}\n", self.regressed));
        out.push_str(&format!(
            "context_gain_rate:     {:.4}\n",
            self.context_gain()
        ));
        out.push_str(&format!(
            "harmful_reorder_rate:  {:.4}\n",
            self.harmful_reorder_rate()
        ));
        out.push_str(&format!("net_gain:              {:+.4}\n", self.net_gain()));
        // 逐 case 变化(只列发生变化者,保持输出精简且确定性)。
        for (id, delta) in &self.deltas {
            let label = match delta {
                ScoreDelta::Improved => "improved",
                ScoreDelta::Regressed => "regressed",
                ScoreDelta::Unchanged => continue,
            };
            out.push_str(&format!("  {id}: {label}\n"));
        }
        out
    }
}

/// 比较两次 benchmark 执行(必须来自同一 suite)。
///
/// 仅在两侧都有判定的 `case_id` 上比较;缺失的 case(不应发生,因为
/// 同一 suite)不参与分母,避免把缺口当成增益。
pub fn compare_runs(baseline: &BenchmarkRun, contextual: &BenchmarkRun) -> ScorerComparison {
    let mut deltas = BTreeMap::new();
    let mut baseline_top1 = 0;
    let mut contextual_top1 = 0;
    let mut improved = 0;
    let mut regressed = 0;
    let mut compared = 0;

    for (id, base) in baseline.outcomes() {
        let Some(ctx) = contextual.outcomes().get(id) else {
            continue;
        };
        compared += 1;
        baseline_top1 += usize::from(base.path_correct);
        contextual_top1 += usize::from(ctx.path_correct);
        let delta = match (base.path_correct, ctx.path_correct) {
            (false, true) => {
                improved += 1;
                ScoreDelta::Improved
            }
            (true, false) => {
                regressed += 1;
                ScoreDelta::Regressed
            }
            _ => ScoreDelta::Unchanged,
        };
        deltas.insert(id.clone(), delta);
    }

    ScorerComparison {
        baseline_scorer: baseline.report().scorer,
        contextual_scorer: contextual.report().scorer,
        baseline_top1,
        contextual_top1,
        improved,
        regressed,
        compared,
        deltas,
    }
}

/// 单条 case 的判定解包(便于调用方直接断言,不必知道 `BenchmarkRun` 形状)。
pub fn outcome_of<'a>(run: &'a BenchmarkRun, case_id: &str) -> Option<&'a CaseOutcome> {
    run.outcomes().get(case_id)
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        return 0.0;
    }
    numerator as f64 / denominator as f64
}
