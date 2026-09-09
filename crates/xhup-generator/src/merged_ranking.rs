//! 单字/词语同码共存时的跨表合并排名(v2 扩展:简码块权重仲裁)。
//!
//! 二字词的全码(4 键)与规范单字全码共享同一个码空间:词码按逐字双拼
//! 拼接推导,可能与某个单字的「音码 + 形码」全码相同(例如 什么 = ufme,
//! 与生僻字 𬳽 的全码相同)。碰撞**不排除词语**——词与单字在同码上合法
//! 共存,碰撞只影响候选排序。
//!
//! 本模块是唯一承担跨表排序仲裁的地方。v2 简码替换后仲裁两类碰撞:
//!
//! ```text
//! A. v2 简码码位(2..5 键)与 baseline 固定候选(单字 2/3/4 码、固定词
//!    4/6/8 键)混排:
//!    → FIXED_FIRST 的绝对位次恒为 1;PRIMARY 同时携带 v2 块内稠密
//!      相对 rank 与 optimizer 导出的绝对 merged_rank;
//!    → v2 条目占据指定绝对位次,baseline 候选按既有相对次序填空;
//!    → 全组指派跨表唯一整数权重(组内 N..1)。
//! B. 无 v2 条目的 4 键字词碰撞码(原语义,不变):
//!    → 组内合并排名(聚合分数降序,文本 Unicode 升序决胜)
//!    → 指派跨表唯一权重(组内 N..1)。
//! ```
//!
//! 两类词典在 Rime 中经 `import_tables` 合入同一 table,同码候选顺序完全
//! 由显式权重决定;合并排名保证碰撞码上不存在跨表同权重平局,使真实部署
//! 的候选顺序与优化器建模一致。非碰撞码的权重保持各表独立的组内 N..1
//! 不变(字节级兼容)。
//!
//! 不使用浮点 quality、导入顺序或同权重平局表达语义。PRIMARY 与
//! FIXED_FIRST 两个投影词典均由顶层静态词典导入,同属禁用 userdb 的
//! table translator;Flow/学习 translator 的质量栅栏不影响静态内部次序。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use xhup_core::KeySequence;

use crate::{char_codes, word_codes};

/// 一条未加权的 4 键排名输入:码、文本(单字或词)、聚合频率分数。
pub(crate) struct ScoredEntry {
    pub(crate) code: KeySequence,
    pub(crate) text: String,
    pub(crate) score: u64,
}

/// 碰撞码上的合并权重覆盖表:`(码, 文本) -> 跨表唯一权重`。
struct Overrides {
    weights: BTreeMap<(KeySequence, String), u32>,
}

fn overrides() -> &'static Overrides {
    static OVERRIDES: OnceLock<Overrides> = OnceLock::new();
    OVERRIDES.get_or_init(build)
}

fn build() -> Overrides {
    let mut weights = BTreeMap::new();

    // ---- A. v2 简码码位:按绝对位次插入,baseline 保持相对次序 ----
    // (merged_rank, PRIMARY 相对 rank;FF 用 0,文本)。
    let mut v2_by_code: BTreeMap<KeySequence, Vec<(usize, usize, String)>> = BTreeMap::new();
    for (word, code, merged_rank) in crate::fixed_first_shortcuts::raw_ranking_entries() {
        v2_by_code
            .entry(code)
            .or_default()
            .push((merged_rank, 0, word));
    }
    for (word, code, rank, merged_rank) in crate::primary_shortcuts::raw_ranking_entries() {
        v2_by_code
            .entry(code)
            .or_default()
            .push((merged_rank, rank, word));
    }

    // 未加权快照避免 merged_ranking ↔ finalized entries 的初始化环。
    let baseline_entries: Vec<ScoredEntry> = char_codes::scored_all_entries()
        .into_iter()
        .chain(word_codes::scored_all_entries())
        .collect();
    let extension_entries = char_codes::scored_extension_entries();
    let mut extensions_by_code: BTreeMap<KeySequence, Vec<(u64, String)>> = BTreeMap::new();
    for entry in &extension_entries {
        extensions_by_code
            .entry(entry.code.clone())
            .or_default()
            .push((entry.score, entry.text.clone()));
    }
    let mut baseline_by_code: BTreeMap<KeySequence, Vec<(u64, String)>> = BTreeMap::new();
    for entry in &baseline_entries {
        baseline_by_code
            .entry(entry.code.clone())
            .or_default()
            .push((entry.score, entry.text.clone()));
    }

    let v2_codes: BTreeSet<KeySequence> = v2_by_code.keys().cloned().collect();
    for (code, mut v2_entries) in v2_by_code {
        v2_entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let mut baseline = baseline_by_code.get(&code).cloned().unwrap_or_default();
        baseline.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let legacy_total = v2_entries.len() + baseline.len();
        let mut slots: Vec<Option<String>> = vec![None; legacy_total];
        for (merged_rank, _, word) in v2_entries {
            assert!(
                (1..=legacy_total).contains(&merged_rank),
                "v2 码 {code} 的绝对位次 {merged_rank} 超出旧候选总数 {legacy_total}: {word}"
            );
            let slot = &mut slots[merged_rank - 1];
            assert!(
                slot.replace(word.clone()).is_none(),
                "v2 码 {code} 的绝对位次 {merged_rank} 重复: {word}"
            );
        }
        let mut baseline = baseline.into_iter();
        for slot in &mut slots {
            if slot.is_none() {
                let (_, text) = baseline.next().expect("空位数应等于 baseline 候选数");
                *slot = Some(text);
            }
        }
        assert!(
            baseline.next().is_none(),
            "baseline 候选应全部填入绝对位次空位"
        );

        let mut ranked: Vec<String> = slots
            .into_iter()
            .map(|slot| slot.expect("全部绝对位次均应填满"))
            .collect();
        let mut extensions = extensions_by_code.get(&code).cloned().unwrap_or_default();
        extensions.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        for (_, text) in extensions {
            assert!(
                !ranked.contains(&text),
                "v2 码 {code} 的 attested 扩展候选与旧候选重复: {text}"
            );
            ranked.push(text);
        }
        let total = ranked.len();
        for (index, text) in ranked.into_iter().enumerate() {
            let weight = u32::try_from(total - index).expect("同码候选数超出 u32");
            assert!(
                weights
                    .insert((code.clone(), text.clone()), weight)
                    .is_none(),
                "码 {code} 上 (码, 文本) 应唯一: {text}"
            );
        }
    }

    // ---- B. 无 v2 条目的 4 键字词碰撞码(原语义,不变) ----
    let chars = char_codes::scored_four_key_entries();
    let extension_chars: Vec<ScoredEntry> = extension_entries
        .into_iter()
        .filter(|entry| entry.code.len() == 4)
        .collect();
    let words = word_codes::scored_four_key_entries();
    let mut char_codes_present = std::collections::BTreeSet::new();
    for entry in chars.iter().chain(extension_chars.iter()) {
        char_codes_present.insert(entry.code.clone());
    }
    let mut collided: Vec<KeySequence> = Vec::new();
    for entry in &words {
        if char_codes_present.contains(&entry.code) && !v2_codes.contains(&entry.code) {
            collided.push(entry.code.clone());
        }
    }
    collided.sort();
    collided.dedup();

    let mut by_code: BTreeMap<KeySequence, Vec<&ScoredEntry>> = BTreeMap::new();
    for entry in chars.iter().chain(words.iter()) {
        by_code.entry(entry.code.clone()).or_default().push(entry);
    }
    for code in &collided {
        let group = by_code.get(code).expect("碰撞码必然同时存在于两侧快照");
        let mut ranked: Vec<&ScoredEntry> = group.clone();
        // 合并排名:聚合分数降序,文本 Unicode 标量升序为最终决胜。
        ranked.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.text.cmp(&b.text)));
        let mut extensions: Vec<&ScoredEntry> = extension_chars
            .iter()
            .filter(|entry| entry.code == *code)
            .collect();
        extensions.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.text.cmp(&b.text)));
        ranked.extend(extensions);
        let group_size = ranked.len();
        for (rank, entry) in ranked.iter().enumerate() {
            let weight = u32::try_from(group_size - rank).expect("同码候选数超出 u32");
            let inserted = weights
                .insert((entry.code.clone(), entry.text.clone()), weight)
                .is_none();
            assert!(
                inserted,
                "碰撞码 {} 上 (码, 文本) 应唯一: {}",
                code, entry.text
            );
        }
    }

    Overrides { weights }
}

/// 查询某条目在碰撞码上的跨表合并权重;非碰撞码返回 `None`(保持表内权重)。
pub(crate) fn merged_weight(code: &KeySequence, text: &str) -> Option<u32> {
    overrides()
        .weights
        .get(&(code.clone(), text.to_string()))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actual_rank(code: &KeySequence, text: &str) -> usize {
        let weight = merged_weight(code, text).expect("条目应参与 merged ranking");
        let max_weight = overrides()
            .weights
            .iter()
            .filter(|((candidate_code, _), _)| candidate_code == code)
            .map(|(_, candidate_weight)| *candidate_weight)
            .max()
            .expect("同码组非空");
        usize::try_from(max_weight - weight + 1).expect("rank 可表示")
    }

    fn all_actual_ranks() -> BTreeMap<(KeySequence, String), usize> {
        let mut max_by_code: BTreeMap<&KeySequence, u32> = BTreeMap::new();
        for ((code, _), &weight) in &overrides().weights {
            max_by_code
                .entry(code)
                .and_modify(|known| *known = (*known).max(weight))
                .or_insert(weight);
        }
        overrides()
            .weights
            .iter()
            .map(|((code, text), &weight)| {
                (
                    (code.clone(), text.clone()),
                    usize::try_from(max_by_code[code] - weight + 1).expect("rank 可表示"),
                )
            })
            .collect()
    }

    #[test]
    fn shenme_coexists_with_rare_char_and_ranks_first() {
        // 冻结回归哨兵:什么(ufme)不得因与生僻字 𬳽 同码而被排除;
        // 频率证据悬殊,什么 必须排在 𬳽 之前。
        let code: KeySequence = "ufme".parse().expect("ufme 是合法键序");
        let word_weight = merged_weight(&code, "什么").expect("什么 必须在词层且参与合并排名");
        let char_weight = merged_weight(&code, "𬳽").expect("𬳽 必须在字层且参与合并排名");
        assert!(
            word_weight > char_weight,
            "什么({word_weight}) 应排在 𬳽({char_weight}) 之前"
        );
    }

    #[test]
    fn merged_weights_are_unique_and_dense_per_collided_code() {
        // 每个碰撞码上的跨表权重恰好是 1..=n 的一个排列(无平局、无缺号):
        // Rime by_weight 排序在唯一权重下是严格全序,真实菜单与分析器逐码一致。
        let overrides = overrides();
        let mut by_code: BTreeMap<&KeySequence, Vec<u32>> = BTreeMap::new();
        for ((code, _), &weight) in &overrides.weights {
            by_code.entry(code).or_default().push(weight);
        }
        assert!(!by_code.is_empty(), "当前规范数据应存在碰撞码(如 ufme)");
        for (code, mut weights) in by_code {
            weights.sort_unstable();
            let expected: Vec<u32> = (1..=weights.len() as u32).collect();
            assert_eq!(weights, expected, "{code} 合并权重应为 1..=n 排列");
        }
    }

    #[test]
    fn non_collided_codes_have_no_override() {
        // 抽样一个不碰撞的 4 键字码(无任何词码命中):jumk 是测试锚定的纯单字全码。
        let code: KeySequence = "jumk".parse().expect("jumk 是合法键序");
        let overrides = overrides();
        assert!(
            overrides.weights.keys().all(|(c, _)| c != &code),
            "非碰撞码不应有合并权重覆盖"
        );
    }

    #[test]
    fn fixed_first_rank_one_outranks_baseline_on_occupied_code() {
        // v2 首选哨兵:uij(时间 的 v2 rank 1 码,与单字碰撞)上
        // 时间 必须排在全部 baseline 候选(铈/鼫)之前。
        let code: KeySequence = "uij".parse().expect("uij 是合法键序");
        let shijian = merged_weight(&code, "时间").expect("时间 应参与合并排名");
        let shi = merged_weight(&code, "铈").expect("铈 应参与合并排名");
        assert!(
            shijian > shi,
            "时间({shijian}) 应排在 baseline 候选 铈({shi}) 之前"
        );
    }

    #[test]
    fn every_v2_entry_reaches_its_optimizer_merged_rank() {
        let ranks = all_actual_ranks();
        for entry in crate::primary_shortcuts::canonical_primary_shortcut_entries() {
            assert_eq!(
                ranks.get(&(entry.shortcut_code().clone(), entry.word().to_string())),
                Some(&entry.merged_rank()),
                "{} {} 应命中 optimizer 绝对位次",
                entry.word(),
                entry.shortcut_code()
            );
        }
        for entry in crate::fixed_first_shortcuts::canonical_fixed_first_shortcut_entries() {
            assert_eq!(
                ranks.get(&(entry.shortcut_code().clone(), entry.word().to_string())),
                Some(&1),
                "{} {} 应为 FIXED_FIRST rank 1",
                entry.word(),
                entry.shortcut_code()
            );
        }
    }

    #[test]
    fn traditional_alias_sentinels_are_runtime_rank_one() {
        for (word, code) in [
            ("就是", "jqu"),
            ("知道", "vdc"),
            ("不是", "buu"),
            ("你们", "nim"),
            ("还是", "hdu"),
            ("因为", "yww"),
            ("如果", "rgo"),
        ] {
            let code: KeySequence = code.parse().expect("哨兵码合法");
            assert_eq!(actual_rank(&code, word), 1, "{word}={code} 必须是静态首选");
        }
    }
}
