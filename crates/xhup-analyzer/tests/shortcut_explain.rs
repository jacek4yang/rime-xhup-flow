//! 简码提示解释卡的契约测试(Issue #83 §3/§7)。
//!
//! 断言的是**解释卡与真实菜单的一致性**,以及 §7 的 ASCII 呈现准则 ——
//! 不是特定词的具体码位。

use xhup_analyzer::shortcut_audit::ShortcutVerdict;
use xhup_analyzer::shortcut_explain::explain_shortcut_hint;

#[test]
fn card_reflects_real_menu_rank_and_fanout() {
    // 解释卡的 rank/fanout 必须来自真实菜单(占位与 rank 一致),
    // 否则 Trainer 会显示与用户所见不符的诊断。
    let hint = explain_shortcut_hint("我们").expect("高频词应有提示");
    match hint.menu_rank {
        Some(rank) => {
            assert!(rank >= 1, "rank 为 1 起");
            assert!(
                rank <= hint.menu_fanout,
                "rank 不得超过该码菜单扇出({rank} vs {})",
                hint.menu_fanout
            );
            assert!(hint.menu_fanout >= 1, "词在菜单内则扇出至少 1");
        }
        None => {
            assert_eq!(hint.menu_fanout, 0, "词不在菜单时扇出必须为 0");
        }
    }
}

#[test]
fn verdict_matches_rank_semantics() {
    // §3 判定必须与 rank 严格对应,不得出现「rank 1 却判 SELECT」之类的矛盾。
    let hint = explain_shortcut_hint("我们").expect("高频词应有提示");
    match hint.menu_rank {
        Some(1) => assert_eq!(hint.verdict, ShortcutVerdict::Useful),
        Some(_) => assert_eq!(hint.verdict, ShortcutVerdict::UsefulWithSelection),
        None => assert_eq!(hint.verdict, ShortcutVerdict::Misleading),
    }
    assert_eq!(hint.is_useful(), hint.verdict == ShortcutVerdict::Useful);
}

#[test]
fn keystrokes_saved_is_never_negative() {
    // 提示视图保证简码严格短于全码;解释卡不得出现负节省。
    for word in ["我们", "时间", "工作", "问题"] {
        if let Some(hint) = explain_shortcut_hint(word) {
            assert!(
                hint.shortcut_len <= hint.full_code_len,
                "{word}: 简码不得长于全码"
            );
            assert!(hint.keystrokes_saved() <= hint.full_code_len);
        }
    }
}

#[test]
fn card_labels_are_ascii_apart_from_the_queried_word() {
    // §7:候选注释与诊断标签极简纯 ASCII。查询词本身是用户内容(中文),
    // 必然出现在卡里;除它之外必须是 ASCII。
    for word in ["我们", "时间", "工作"] {
        if let Some(hint) = explain_shortcut_hint(word) {
            let card = hint.render_card();
            let labels = card.replace(word, "");
            assert!(
                labels.is_ascii(),
                "{word}: 除查询词外必须纯 ASCII:{labels:?}"
            );
            assert!(labels.contains("verdict"), "卡片必须含判定行");
            assert!(labels.contains("menu rank"), "卡片必须含菜单 rank 行");
        }
    }
}

#[test]
fn unknown_and_empty_queries_return_none() {
    assert_eq!(explain_shortcut_hint(""), None);
    assert_eq!(explain_shortcut_hint("  "), None);
    assert_eq!(explain_shortcut_hint("xyznotaword"), None);
}

#[test]
fn explanation_is_deterministic() {
    let a = explain_shortcut_hint("我们");
    let b = explain_shortcut_hint("我们");
    assert_eq!(a, b, "同一词必须得到同一解释");
    // 前后空白应被忽略(与 explain_production_word 同口径)。
    assert_eq!(explain_shortcut_hint("  我们  "), a);
}

#[test]
fn every_advertised_hint_explains_without_contradiction() {
    // 抽样一批真实词:凡是**有提示**的词,解释卡都必须自洽可渲染;
    // 凡是无提示的,必须返回 None(不得伪造空卡)。
    let candidates = [
        "我们", "时间", "工作", "问题", "时候", "什么", "怎么", "可以", "一个", "没有",
    ];
    let mut explained = 0usize;
    for word in candidates {
        if let Some(hint) = explain_shortcut_hint(word) {
            explained += 1;
            assert_eq!(hint.word, word);
            assert!(!hint.shortcut_code.is_empty(), "{word}: 提示码不得为空");
            assert!(!hint.render_card().is_empty());
            // 判定与 rank 必须一致(再次校验,防止未来改动破坏不变量)。
            match hint.menu_rank {
                Some(1) => assert_eq!(hint.verdict, ShortcutVerdict::Useful),
                Some(_) => assert_eq!(hint.verdict, ShortcutVerdict::UsefulWithSelection),
                None => assert_eq!(hint.verdict, ShortcutVerdict::Misleading),
            }
        }
    }
    assert!(explained > 0, "至少应有部分高频词带提示");
}
