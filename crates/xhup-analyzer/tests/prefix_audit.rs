//! 词语简码层 prefix 拓扑审计测试:全量统计回归锚点、哨兵确定性与结构不变量。

use xhup_analyzer::occupancy::CodeOccupancy;
use xhup_analyzer::prefix::audit_prefix_topology;
use xhup_generator::{canonical_fixed_first_shortcut_entries, canonical_primary_shortcut_entries};

#[test]
fn prefix_topology_counts_match_frozen_audit() {
    // 回归锚点:数值来自 optimizer v2 canonical data 的全量静态审计。
    let baseline = CodeOccupancy::build_baseline_fixed();
    let audit = audit_prefix_topology(&baseline);
    assert_eq!(
        audit.shortcut_count,
        canonical_primary_shortcut_entries().len() + canonical_fixed_first_shortcut_entries().len()
    );
    assert_eq!(audit.shortcut_count, 68_842);
    assert_eq!(audit.shortcut_prefix_of_baseline_pairs, 82_879);
    assert_eq!(audit.shortcuts_prefixing_baseline, 9_770);
    assert_eq!(audit.baseline_prefix_of_shortcut_pairs, 138_747);
    assert_eq!(audit.shortcut_to_shortcut_pairs, 61_460);
}

#[test]
fn per_length_sentinels_are_deterministic_and_wellformed() {
    let baseline = CodeOccupancy::build_baseline_fixed();
    let first = audit_prefix_topology(&baseline);
    let second = audit_prefix_topology(&baseline);
    assert_eq!(first.lengths.len(), 4, "覆盖 2~5 键四层");
    for (a, b) in first.lengths.iter().zip(second.lengths.iter()) {
        assert_eq!(a.length, b.length);
        assert_eq!(a.rows, b.rows);
        let key = |s: &Option<xhup_analyzer::PrefixSentinel>| {
            s.as_ref()
                .map(|s| (s.word.clone(), s.shortcut_code.to_string()))
        };
        assert_eq!(key(&a.lex_first), key(&b.lex_first), "哨兵必须确定");
        assert_eq!(key(&a.top_frequency), key(&b.top_frequency));
        assert_eq!(key(&a.prefix_lex_first), key(&b.prefix_lex_first));
        assert_eq!(key(&a.non_prefix_lex_first), key(&b.non_prefix_lex_first));
    }

    // 结构不变量:非空层必有 lex-first 与 top-frequency 哨兵;prefix 哨兵的
    // shortcut 必须是自身完整码的 strict prefix,非 prefix 哨兵反之。
    for length in &first.lengths {
        if length.rows == 0 {
            assert!(length.lex_first.is_none());
            continue;
        }
        let lex = length.lex_first.as_ref().expect("非空层必有首条");
        assert_eq!(lex.shortcut_code.len(), length.length);
        let top = length.top_frequency.as_ref().expect("非空层必有高频哨兵");
        assert!(
            top.frequency_score >= lex.frequency_score,
            "top-frequency 哨兵频率不得低于首条"
        );
        if let Some(prefix) = &length.prefix_lex_first {
            assert!(
                prefix.shortcut_code.len() < prefix.full_code.len()
                    && prefix
                        .full_code
                        .as_slice()
                        .starts_with(prefix.shortcut_code.as_slice()),
                "{} 必须是自身完整码的 strict prefix",
                prefix.shortcut_code
            );
        }
        if let Some(non_prefix) = &length.non_prefix_lex_first {
            assert!(
                !non_prefix
                    .full_code
                    .as_slice()
                    .starts_with(non_prefix.shortcut_code.as_slice()),
                "{} 不得是自身完整码的 prefix",
                non_prefix.shortcut_code
            );
        }
    }
}

/// 每层哨兵选择的冻结值(canonical data 变更时必须人工 review 此处)。
#[test]
fn sentinel_frozen_values() {
    let baseline = CodeOccupancy::build_baseline_fixed();
    let audit = audit_prefix_topology(&baseline);
    let frozen: [(usize, &str, &str, &str); 4] = [
        (2, "暗部", "anbu", "ab"),
        (3, "啊啊啊", "aaaaaa", "aaa"),
        (4, "安安静静", "ananjkjk", "aajj"),
        (5, "阿卜杜拉", "aabodula", "aabdl"),
    ];
    for (length, word, full, shortcut) in frozen {
        let slot = &audit.lengths[length - 2];
        let lex = slot.lex_first.as_ref().expect("层非空");
        assert_eq!(lex.word, word);
        assert_eq!(lex.full_code.to_string(), full);
        assert_eq!(lex.shortcut_code.to_string(), shortcut);
    }
    let top: [(usize, &str, &str); 4] = [
        (2, "一个", "yg"),
        (3, "就是", "jqu"),
        (4, "这样的", "veyd"),
        (5, "实际上", "uijiu"),
    ];
    for (length, word, shortcut) in top {
        let slot = &audit.lengths[length - 2];
        let sentinel = slot.top_frequency.as_ref().expect("层非空");
        assert_eq!(sentinel.word, word);
        assert_eq!(sentinel.shortcut_code.to_string(), shortcut);
    }
}
