//! Prefix-Space Compiler v3 的生产数据接入(Issue #83 §25 第 4 步 / §2 / §7)。
//!
//! 从真实 canonical 词集构建有界 [`PrefixTarget`] 宇宙并播种一级简码冻结
//! 锚点,供求解器做指标/解释实验。本模块:
//!
//! - **不**导出新的 canonical 映射;
//! - **不**改写 `word_shortcuts_primary.tsv` / `word_fixed_first.tsv`;
//! - **不**改动 `xhup_flow_static` 或生成器词典。
//!
//! 生产 mapping 仍是冻结的 optimizer v2 canonical。
//!
//! # 质量映射
//!
//! [`LexicalEvidence::daily_prior`] 是多源融合后的 **log 域相对值**
//! (0 ≈ 各源中位水平,可略为负)。求解器需要严格为正的 `mass`:
//!
//! - 优先 `daily_prior`: `mass = exp(prior).clamp(MIN, MAX)`。中位(~0)
//!   映射为 1;中位以下落在 `(0, 1)`;中位以上 `> 1`。指数映射保持排序
//!   且永不产生非正质量。
//! - 先验缺失时回退 `ln_1p(normalized_frequency)`,并地板到 `MIN`。
//!
//! # 目标选取
//!
//! 候选宇宙与 v2 评估管线一致:[`crate::sweep_v2::v2_targets`]
//! (monotone-v2 ∪ legacy-v1 并集)。`limit` 按与
//! `sweep_v2::frequency_order` 相同的确定性口径截取:
//! **万象 `frequency_score` 降序,词形升序**。
//!
//! `limit = None` 表示全量(研究用,CI 禁止)。测试与 CLI 冒烟必须传入
//! 显式上界。

use std::collections::{BTreeMap, BTreeSet};

use xhup_core::KeySequence;
use xhup_generator::canonical_level1_shortcuts;

use crate::candidates::WordTarget;
use crate::evidence::{LexicalEvidence, LexicalEvidenceSet};

use super::slot::{SlotCandidate, SlotPlacementSource};
use super::solver::{PrefixSpaceCompiledModel, PrefixSpaceStats, PrefixTarget};
use super::trie::PrefixTrie;

/// CLI `--production` 未给 `--limit` 时的默认上界。
pub const DEFAULT_PRODUCTION_LIMIT: usize = 256;

/// 集成测试使用的有界规模(必须在数秒内完成;显式传入,禁止默认全量)。
pub const TEST_PRODUCTION_LIMIT: usize = 32;

/// 求解器质量下限(严格为正,避免 0/NaN 破坏排序)。
const MIN_SOLVER_MASS: f64 = 1e-9;

/// 求解器质量上限(防止极端先验撑爆效用)。
const MAX_SOLVER_MASS: f64 = 1e6;

/// 一级简码冻结锚点的占位质量(与 log 域中位 `exp(0) = 1` 对齐)。
/// 冻结槽位排序优先于质量,此值只参与子树拥塞统计。
const LEVEL1_ANCHOR_MASS: f64 = 1.0;

/// 真实生产词集上的有界前缀空间输入。
#[derive(Clone, Debug)]
pub struct ProductionPrefixUniverse {
    /// 按频率序截取后的求解目标。
    pub targets: Vec<PrefixTarget>,
    /// 仅含一级简码冻结锚点的初始 Trie(不含 8105 单字全表)。
    pub initial_trie: PrefixTrie,
}

/// 从 canonical 分析投影构建有界生产宇宙。
///
/// `limit = Some(n)` 取频率序前 n 个词;`None` 为全量,仅供显式研究
/// 调用,测试不得使用。
pub fn build_production_universe(limit: Option<usize>) -> ProductionPrefixUniverse {
    let data = crate::build_analysis();
    let evidence_set = LexicalEvidenceSet::build(&data.words, &data.frequency);
    let evidence = crate::sweep_v2::evidence_by_word(&evidence_set);
    let tradition = crate::sweep_v2::tradition_map();
    let mut word_targets = crate::sweep_v2::v2_targets(&data.words);
    sort_by_frequency_order(&mut word_targets);
    // 解释卡以词形为键;同词多码时保留频率序中的第一条,再截取 limit。
    let mut seen_words = BTreeSet::new();
    word_targets.retain(|t| seen_words.insert(t.word().to_string()));
    if let Some(n) = limit {
        word_targets.truncate(n);
    }

    let targets: Vec<PrefixTarget> = word_targets
        .iter()
        .map(|target| prefix_target_from_word(target, &evidence, &tradition))
        .collect();

    ProductionPrefixUniverse {
        targets,
        initial_trie: seed_level1_trie(),
    }
}

/// 与 `sweep_v2::frequency_order` 相同的确定性全序:
/// `frequency_score` 降序,词形升序。
fn sort_by_frequency_order(targets: &mut [WordTarget]) {
    targets.sort_by(|a, b| {
        b.frequency_score()
            .cmp(&a.frequency_score())
            .then(a.word().cmp(b.word()))
    });
}

fn prefix_target_from_word(
    target: &WordTarget,
    evidence: &BTreeMap<String, LexicalEvidence>,
    tradition: &BTreeMap<String, KeySequence>,
) -> PrefixTarget {
    let mass = evidence
        .get(target.word())
        .map(solver_mass)
        .unwrap_or(MIN_SOLVER_MASS);
    PrefixTarget {
        text: target.word().to_string(),
        full_code: target.full_code().clone(),
        mass,
        legal_candidates: target
            .candidates()
            .iter()
            .map(|c| (c.shortcut_code().clone(), c.mode().pattern()))
            .collect(),
        // PRIMARY 优先,其次 FIXED_FIRST;与 tradition_map 一致,不发明码。
        legacy_code: tradition.get(target.word()).cloned(),
    }
}

/// 将词汇证据映射为求解器正质量。见模块文档「质量映射」。
pub fn solver_mass(evidence: &LexicalEvidence) -> f64 {
    let raw = match evidence.daily_prior() {
        Some(prior) => prior.exp().clamp(MIN_SOLVER_MASS, MAX_SOLVER_MASS),
        None => evidence.normalized_frequency().ln_1p().max(MIN_SOLVER_MASS),
    };
    if raw.is_finite() && raw > 0.0 {
        raw
    } else {
        MIN_SOLVER_MASS
    }
}

/// 仅播种 26 个一级简码冻结锚点,不把 8105 单字全表倒入 Trie。
fn seed_level1_trie() -> PrefixTrie {
    let mut trie = PrefixTrie::new();
    for entry in canonical_level1_shortcuts() {
        let code = KeySequence::from_keys(&[entry.key()]).expect("一级简码恰好一键");
        trie.insert_candidate(
            &code,
            SlotCandidate::new(
                entry.hanzi().to_string(),
                code.clone(),
                1,
                SlotPlacementSource::Level1Frozen,
                LEVEL1_ANCHOR_MASS,
                true,
            ),
        );
    }
    trie.recompute_subtree_stats();
    trie
}

/// 生产求解不变量(前缀闭合、全码仍合法、目标均有解释)。
///
/// 不写任何映射文件。失败时返回第一条违规描述。
pub fn verify_production_invariants(
    targets: &[PrefixTarget],
    model: &PrefixSpaceCompiledModel,
) -> Result<(), String> {
    verify_prefix_closed(&model.trie)?;
    verify_level1_frozen_anchors(&model.trie)?;
    for target in targets {
        let exp = model
            .explanations
            .get(&target.text)
            .ok_or_else(|| format!("target {} silently dropped (no explanation)", target.text))?;
        if !exp.legal_codes.iter().any(|c| c == &target.full_code) {
            return Err(format!(
                "target {} lost full_code {} from legal set",
                target.text, target.full_code
            ));
        }
        if !(exp.frequency_mass.is_finite() && exp.frequency_mass > 0.0) {
            return Err(format!("target {} has non-positive mass", target.text));
        }
        // 简码放置必须落在已创建节点;未放置则保留全码合法性(解释卡)。
        if let Some(code) = &exp.selected_code
            && model.trie.get_node(code).is_none()
        {
            return Err(format!(
                "target {} selected {} but node is missing",
                target.text, code
            ));
        }
    }
    if model.stats.total_targets != targets.len() {
        return Err(format!(
            "stats.total_targets {} != input {}",
            model.stats.total_targets,
            targets.len()
        ));
    }
    Ok(())
}

/// 若码长 L 的节点存在,则求解器创建的全部真前缀节点也必须存在。
pub fn verify_prefix_closed(trie: &PrefixTrie) -> Result<(), String> {
    for idx in 0..trie.len() {
        let Some(code) = trie.node(idx).code.clone() else {
            continue;
        };
        let keys = code.as_slice();
        for end in 1..keys.len() {
            let prefix = KeySequence::from_keys(&keys[..end])
                .expect("既有节点的非空真前缀可构造 KeySequence");
            if trie.get_node(&prefix).is_none() {
                return Err(format!(
                    "prefix-closed violated: {code} exists but prefix {prefix} is missing"
                ));
            }
        }
    }
    Ok(())
}

fn verify_level1_frozen_anchors(trie: &PrefixTrie) -> Result<(), String> {
    for entry in canonical_level1_shortcuts() {
        let code = KeySequence::from_keys(&[entry.key()]).expect("一级简码恰好一键");
        let node = trie
            .get_node(&code)
            .ok_or_else(|| format!("level-1 frozen anchor {} missing after solve", entry.key()))?;
        let ok = node.slots.iter().any(|s| {
            s.source == SlotPlacementSource::Level1Frozen
                && s.is_frozen
                && s.text == entry.hanzi().to_string()
        });
        if !ok {
            return Err(format!(
                "level-1 frozen slot missing for {} ({})",
                entry.key(),
                entry.hanzi()
            ));
        }
    }
    Ok(())
}

/// 比较两次求解的聚合统计,忽略运行耗时(毫秒级抖动)。
pub fn stats_equal_except_runtime(a: &PrefixSpaceStats, b: &PrefixSpaceStats) -> bool {
    let mut left = a.clone();
    let mut right = b.clone();
    left.solver_time_ms = 0;
    right.solver_time_ms = 0;
    left == right
}
