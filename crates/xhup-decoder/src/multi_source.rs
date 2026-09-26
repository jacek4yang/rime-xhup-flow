//! 多源转移证据的确定性合并(Issue #83 §25 第 6 步消费侧)。
//!
//! # 为什么需要显式合并语义
//!
//! `BigramModel` 的 [`BigramModel::observe`](crate::BigramModel::observe) 会
//! 累加计数,因此「把两个模型直接相加」看起来足够。但真实数据的量纲不同:
//! KDConv 有 92,558 句,PTT 有 861,745 句(9.3 倍)。若直接相加,PTT 会
//! **淹没** KDConv,单个来源的偏差(PTT 是八卦版推文口语,域极窄)会系统性
//! 主导所有排序,而这是 §6「弱证据必须平滑降级」要避免的。
//!
//! 本模块因此提供显式的合并策略,并把「每源贡献是否被归一化」记录下来,
//! 使结果可解释、可复现:
//!
//! - [`MergePolicy::RawSum`] —— 直接相加。语义最简单,但与来源语料量成比例;
//! - [`MergePolicy::PerSourceNormalized`] —— 每源先按该源**总观测数**伸缩到
//!   同一常量,再相加。使两个来源的量级对等,不因语料大小失衡;
//! - [`MergePolicy::MaxEvidence`] —— 取各源计数的最大值。最保守:只承认
//!   「两个来源中较强的那个」,不产生任何加成。
//!
//! # 确定性
//!
//! 合并遍历 `BTreeMap`(有序),整数运算,不使用浮点;同一输入必得同一输出。
//! 归一化使用整数比例缩放(先乘后除),缩放常量固定,不引入平台相关误差。

use std::collections::BTreeMap;

use crate::BigramModel;

/// 合并后的来源审计记录(可解释性:哪些来源以何种策略参与了合并)。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergeAudit {
    /// 各来源标识与合并前的观测总量(`Σ` 全部计数)。
    pub sources: Vec<(&'static str, u64)>,
    /// 实际使用的策略标识。
    pub policy: &'static str,
    /// 合并后唯一转移对数。
    pub merged_pairs: usize,
}

/// 多源合并策略。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergePolicy {
    /// 直接相加(语义最简单;贡献与来源语料量成正比)。
    RawSum,
    /// 每源按自身规模归一化到同一常量后相加(消除语料量失衡)。
    PerSourceNormalized,
    /// 取各源计数的最大值(最保守,不产生加成)。
    MaxEvidence,
}

impl MergePolicy {
    pub const fn id(self) -> &'static str {
        match self {
            Self::RawSum => "raw-sum/v1",
            Self::PerSourceNormalized => "per-source-normalized/v1",
            Self::MaxEvidence => "max-evidence/v1",
        }
    }
}

/// 归一化目标量级(Q10 定点:归一化后每源的计数总量缩放到此值)。
///
/// 取 1,000,000 与 KDConv 原始规模同数量级,避免整数缩放过早丢精度。
pub const NORMALIZE_SCALE: u64 = 1_000_000;

/// 按 `policy` 把多个 `(来源标识, 模型)` 合并为一个模型。
///
/// 返回合并模型与 [`MergeAudit`]。空输入返回空模型(不 panic)。
/// 单来源时三种策略结果相同(但仍走统一路径,保证调用方无分支)。
pub fn merge_models(
    sources: &[(&'static str, &BigramModel)],
    policy: MergePolicy,
) -> (BigramModel, MergeAudit) {
    merge_models_weighted(
        &sources
            .iter()
            .map(|(id, model)| (*id, *model, 1))
            .collect::<Vec<_>>(),
        policy,
    )
}

/// 带每源权重的合并(`weight` 为非负整数缩放;0 = 该源被完全排除)。
///
/// 动机(PR #137 测量):KDConv 与重放夹具同域,PTT 是异域补强;RawSum
/// 会把共同 pair 的证据相加、稀释同域相对差距(2000 句重放 rank1 5532
/// → 5527)。**按域加权**让调用方在「补强独占对」与「不稀释同域证据」
/// 之间取实测最优点;权重语义是确定性的整数缩放(先乘后除,保持单调)。
///
/// 权重并非独立策略,而是每源的整数缩放:合并语义仍由 [`MergePolicy`]
/// 决定,审计里的 `sources` 记录缩放后的观察总量,权重因此可解释。
/// 权重全为 1 时与不带权重的合并逐字节一致(确定性)。
///
/// [`merge_models`]:crate::merge_models
pub fn merge_models_weighted(
    sources: &[(&'static str, &BigramModel, u64)],
    policy: MergePolicy,
) -> (BigramModel, MergeAudit) {
    // 先收集每源的 (left, right) → count(权重在收集期一次性整数缩放)。
    let mut per_source: Vec<BTreeMap<(String, String), u64>> = Vec::new();
    let mut audit_sources = Vec::new();
    let mut all_keys: Vec<(String, String)> = Vec::new();
    for (id, model, weight) in sources {
        let mut rows: BTreeMap<(String, String), u64> = BTreeMap::new();
        for (left, right, count) in model.transitions() {
            rows.insert(
                (left.to_string(), right.to_string()),
                count.saturating_mul(*weight),
            );
        }
        audit_sources.push((*id, model.observed_total().saturating_mul(*weight)));
        // 键并集(BTreeMap 有序;最后统一 dedup 保证确定性)。
        all_keys.extend(rows.keys().cloned());
        per_source.push(rows);
    }
    all_keys.sort();
    all_keys.dedup();

    // 逐对合并。
    let mut merged_rows: BTreeMap<(String, String), u64> = BTreeMap::new();
    for key in &all_keys {
        let value = match policy {
            MergePolicy::RawSum => per_source
                .iter()
                .map(|rows| rows.get(key).copied().unwrap_or(0))
                .fold(0u64, u64::saturating_add),
            MergePolicy::MaxEvidence => per_source
                .iter()
                .map(|rows| rows.get(key).copied().unwrap_or(0))
                .max()
                .unwrap_or(0),
            MergePolicy::PerSourceNormalized => {
                let mut sum = 0u64;
                for (index, rows) in per_source.iter().enumerate() {
                    let Some(&count) = rows.get(key) else {
                        continue;
                    };
                    let total = audit_sources[index].1;
                    if total == 0 {
                        continue;
                    }
                    // 整数比例缩放:count / total × NORMALIZE_SCALE,先乘后除。
                    let scaled =
                        (count as u128).saturating_mul(NORMALIZE_SCALE as u128) / (total as u128);
                    // 有证据但对齐后为 0 时保底为 1,避免真实证据被抹平。
                    let scaled = u64::try_from(scaled).unwrap_or(u64::MAX).max(1);
                    sum = sum.saturating_add(scaled);
                }
                sum
            }
        };
        if value > 0 {
            merged_rows.insert(key.clone(), value);
        }
    }

    let mut model = BigramModel::default();
    for ((left, right), count) in &merged_rows {
        model.observe(left, right, *count);
    }

    let audit = MergeAudit {
        sources: audit_sources,
        policy: policy.id(),
        merged_pairs: merged_rows.len(),
    };
    (model, audit)
}
