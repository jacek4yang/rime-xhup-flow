//! 单字/词语同码共存时的跨表合并排名。
//!
//! 二字词的全码(4 键)与规范单字全码共享同一个码空间:词码按逐字双拼
//! 拼接推导,可能与某个单字的「音码 + 形码」全码相同(例如 什么 = ufme,
//! 与生僻字 𬳽 的全码相同)。碰撞**不排除词语**——词与单字在同码上合法
//! 共存,碰撞只影响候选排序。
//!
//! 本模块是唯一承担跨表排序仲裁的地方:
//!
//! ```text
//! char_codes 未加权 4 键条目 + word_codes 未加权 4 键条目
//!     → 找出两侧同时出现的码(碰撞码)
//!     → 组内合并排名(聚合分数降序,文本 Unicode 升序决胜)
//!     → 指派跨表唯一权重(组内 N..1)
//!     → 两个生成器在同码条目上回填该权重
//! ```
//!
//! 两个词典在 Rime 中经 `import_tables` 合入同一 table,同码候选顺序完全由
//! 显式权重决定;合并排名保证碰撞码上不存在跨表同权重平局,使真实部署的
//! 候选顺序与 `xhup-analyzer` 的占用模型逐项一致。非碰撞码的权重保持各表
//! 独立的组内 N..1 不变(字节级兼容)。
//!
//! 排名证据两侧同源(万象 / RIME-LMDG 聚合分数):单字侧按读音聚合,词语侧
//! 按词聚合,分数刻度可比;高频常用词因此自然排在占用同码的生僻字之前。

use std::collections::BTreeMap;
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
    let mut by_code: BTreeMap<KeySequence, Vec<&ScoredEntry>> = BTreeMap::new();
    let chars = char_codes::scored_four_key_entries();
    let words = word_codes::scored_four_key_entries();
    // 词侧同码可能有多条(多词同码),字侧同理;按码汇总后再判断跨表碰撞。
    let mut char_codes_present = std::collections::BTreeSet::new();
    for entry in &chars {
        char_codes_present.insert(entry.code.clone());
    }
    let mut collided: Vec<KeySequence> = Vec::new();
    for entry in &words {
        if char_codes_present.contains(&entry.code) {
            collided.push(entry.code.clone());
        }
    }
    collided.sort();
    collided.dedup();

    for entry in chars.iter().chain(words.iter()) {
        by_code.entry(entry.code.clone()).or_default().push(entry);
    }

    let mut weights = BTreeMap::new();
    for code in &collided {
        let group = by_code.get(code).expect("碰撞码必然同时存在于两侧快照");
        let mut ranked: Vec<&ScoredEntry> = group.clone();
        // 合并排名:聚合分数降序,文本 Unicode 标量升序为最终决胜。
        ranked.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.text.cmp(&b.text)));
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
}
