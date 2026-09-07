//! 常用词覆盖与可达性 P0 门禁(离线,纯生成器公共 API)。
//!
//! 发布阻断语义(见 docs/input-model-v2.md §7):
//! - top-100 高频词(按万象聚合分数)必须 100% 存在于词词典且编码正确;
//! - top-100 高频词在其码位上的合并排名(字+词跨表合并,merged_ranking)
//!   不得超过第 3 位 —— 高频词不得被生僻同码字/词埋到深处;
//! - top-1000 高频词必须全部在库(词汇存在性,不允许静默丢弃);
//! - 骨干哨兵(什么 ufme / 但是 djui)必须占据碰撞码的第 1 位。

use std::collections::HashMap;

use xhup_generator::{
    generate_rime_char_dictionary, generate_rime_flow_dictionary, generate_rime_word_dictionary,
    word_code_analysis_entries,
};

/// 解析词典文本为 `(词, 码, 权重)` 数据行(跳过 `...` 前的头部)。
fn dict_rows(text: &str) -> Vec<(String, String, u32)> {
    text.lines()
        .skip_while(|line| *line != "...")
        .skip(1)
        .map(|line| {
            let mut fields = line.split('\t');
            let word = fields.next().expect("词字段存在").to_string();
            let code = fields.next().expect("码字段存在").to_string();
            let weight = fields
                .next()
                .expect("权重字段存在")
                .parse()
                .expect("权重为整数");
            (word, code, weight)
        })
        .collect()
}

/// 合并排名视图:码 → 该码上全部(字+词)条目的权重多重集。
///
/// 字词典码长 2/3/4,词词典码长 4/6/8,跨表碰撞只可能发生在 4 键码上
/// (二字词全码 vs 单字全码),merged_ranking 保证同码权重唯一无平局。
fn merged_code_weights() -> HashMap<String, Vec<u32>> {
    let mut by_code: HashMap<String, Vec<u32>> = HashMap::new();
    for text in [
        generate_rime_char_dictionary(),
        generate_rime_word_dictionary(),
    ] {
        for (_, code, weight) in dict_rows(&text) {
            by_code.entry(code).or_default().push(weight);
        }
    }
    for weights in by_code.values_mut() {
        weights.sort_unstable_by(|a, b| b.cmp(a));
    }
    by_code
}

/// 条目在其码位上的合并排名(1 = 首选)。
fn merged_rank(by_code: &HashMap<String, Vec<u32>>, code: &str, weight: u32) -> usize {
    let weights = by_code
        .get(code)
        .unwrap_or_else(|| panic!("码 {code} 应存在于合并视图"));
    weights.iter().filter(|&&w| w > weight).count() + 1
}

/// 按万象聚合分数降序的前 N 个词条目(分数并列时按词字典序,确定性)。
fn top_words(n: usize) -> Vec<(String, String, u64)> {
    let mut entries: Vec<_> = word_code_analysis_entries()
        .into_iter()
        .map(|e| {
            (
                e.word().to_string(),
                e.code().to_string(),
                e.frequency_score(),
            )
        })
        .collect();
    entries.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
    entries.truncate(n);
    entries
}

#[test]
fn top_100_words_present_with_correct_full_code() {
    let word_dict = dict_rows(&generate_rime_word_dictionary());
    for (word, code, _) in top_words(100) {
        assert!(
            word_dict.iter().any(|(w, c, _)| *w == word && *c == code),
            "top-100 高频词 {word} 必须以其全码 {code} 存在于词词典"
        );
    }
}

#[test]
fn top_1000_words_all_present() {
    let present: std::collections::HashSet<String> = dict_rows(&generate_rime_word_dictionary())
        .into_iter()
        .map(|(w, _, _)| w)
        .collect();
    for (word, _, _) in top_words(1000) {
        assert!(
            present.contains(&word),
            "top-1000 高频词 {word} 必须在词库中"
        );
    }
}

#[test]
fn top_100_words_rank_within_top_3() {
    let by_code = merged_code_weights();
    let word_dict = dict_rows(&generate_rime_word_dictionary());
    let weight_of: HashMap<(&str, &str), u32> = word_dict
        .iter()
        .map(|(w, c, weight)| ((w.as_str(), c.as_str()), *weight))
        .collect();
    let mut worst: Vec<(String, usize)> = Vec::new();
    for (word, code, _) in top_words(100) {
        let weight = weight_of[&(word.as_str(), code.as_str())];
        let rank = merged_rank(&by_code, &code, weight);
        if rank > 3 {
            worst.push((word, rank));
        }
    }
    assert!(
        worst.is_empty(),
        "top-100 高频词合并排名不得超过第 3 位,超标: {worst:?}"
    );
}

#[test]
fn collision_sentinels_hold_rank_1() {
    // 回归锚点(历史缺陷样本):碰撞码上高频词必须压过生僻字。
    let by_code = merged_code_weights();
    let word_dict = dict_rows(&generate_rime_word_dictionary());
    for (word, code) in [("什么", "ufme"), ("但是", "djui")] {
        let weight = word_dict
            .iter()
            .find(|(w, c, _)| w == word && c == code)
            .unwrap_or_else(|| panic!("{word} 必须以 {code} 存在于词词典"))
            .2;
        assert_eq!(
            merged_rank(&by_code, code, weight),
            1,
            "{word} 必须占据碰撞码 {code} 的第 1 位"
        );
    }
}

#[test]
fn top_100_flow_weights_equal_frequency_scores() {
    // 组句路径质量门禁:top-100 词在组句词典中的权重即万象分数原值
    // (rime_flow.rs 的频率证据语义,防止权重退化回组内名次)。
    let flow = dict_rows(&generate_rime_flow_dictionary());
    let flow_weight: HashMap<&str, u32> = flow
        .iter()
        .map(|(w, _, weight)| (w.as_str(), *weight))
        .collect();
    for (word, _, score) in top_words(100) {
        assert_eq!(
            flow_weight[word.as_str()],
            u32::try_from(score).expect("分数应可表示为 u32"),
            "组句词典中 {word} 的权重应等于万象聚合分数"
        );
    }
}
