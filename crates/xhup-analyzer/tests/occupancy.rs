//! 码位占用测试:分层行数回归锚点、5/7 键空闲、碰撞分类、扇出哨兵,
//! 以及 baseline fixed 与 current production 两种占用语义的拆分。

use xhup_analyzer::occupancy::{CandidateSource, CodeOccupancy, CollisionClass};
use xhup_core::KeySequence;
use xhup_generator::{canonical_fixed_first_shortcut_entries, canonical_primary_shortcut_entries};

fn code(text: &str) -> KeySequence {
    text.parse().expect("合法码")
}

#[test]
fn layer_rows_match_production_audit() {
    // 回归锚点:与 production 编译表行数审计一致(见 PR #20 审计)。
    // baseline 固定层不含已入库的词语简码层。
    let audit = CodeOccupancy::build_baseline_fixed().layer_audit();
    assert_eq!(audit.level1_shortcut_rows, 26);
    assert_eq!(audit.char_2key_rows, 8573);
    assert_eq!(audit.char_3key_rows, 9022);
    assert_eq!(audit.char_4key_rows, 9158);
    assert_eq!(audit.word_4key_rows, 50000);
    assert_eq!(audit.word_6key_rows, 30000);
    assert_eq!(audit.word_8key_rows, 20000);
    assert_eq!(audit.word_shortcut_rows(), 0);
    assert_eq!(audit.total_rows(), 126779);
}

#[test]
fn five_and_seven_key_spaces_are_currently_empty() {
    let occupancy = CodeOccupancy::build_baseline_fixed();
    let stats = occupancy.length_stats();
    assert_eq!(stats.len(), 8, "统计覆盖 1..=8 键");
    assert_eq!(stats[4].length(), 5);
    assert_eq!(stats[4].rows(), 0, "5-key baseline 应为空闲空间");
    assert_eq!(stats[4].distinct_codes(), 0);
    assert_eq!(stats[6].length(), 7);
    assert_eq!(stats[6].rows(), 0, "7-key baseline 应为空闲空间");
    // 任意 5/7 键码的碰撞分类为 None
    assert_eq!(
        occupancy.collision_class(&code("qqqqq")),
        CollisionClass::None
    );
    assert_eq!(
        occupancy.collision_class(&code("qqqqqqq")),
        CollisionClass::None
    );
}

#[test]
fn collision_classes_are_correct() {
    let occupancy = CodeOccupancy::build_baseline_fixed();
    assert_eq!(
        occupancy.collision_class(&code("q")),
        CollisionClass::Level1
    );
    assert_eq!(
        occupancy.collision_class(&code("yi")),
        CollisionClass::Char2Key
    );
    assert_eq!(
        occupancy.collision_class(&code("jid")),
        CollisionClass::Char3Key
    );
    assert_eq!(
        occupancy.collision_class(&code("jumk")),
        CollisionClass::FullCodeChar
    );
    assert_eq!(
        occupancy.collision_class(&code("womf")),
        CollisionClass::FixedWord
    );
    assert_eq!(
        occupancy.collision_class(&code("uurufa")),
        CollisionClass::FixedWord
    );
}

#[test]
fn fanout_and_rank_sentinels() {
    let occupancy = CodeOccupancy::build_baseline_fixed();
    for (text, expected) in [("yi", 136), ("jid", 14), ("jumk", 5)] {
        assert_eq!(occupancy.fanout(&code(text)), expected, "{text} 扇出");
    }
    // 组内名次与显式权重一致:rank 1 权重最大,且权重 = fanout − rank + 1
    let group = occupancy.group(&code("jumk")).expect("jumk 组存在");
    for candidate in group {
        assert_eq!(
            candidate.rime_weight() as usize,
            group.len() - candidate.rank() as usize + 1,
            "jumk 组内权重与名次应互补"
        );
    }
    // 一级简码:q → 去,rank 1
    let level1 = occupancy.group(&code("q")).expect("q 组存在");
    assert_eq!(level1.len(), 1);
    assert_eq!(level1[0].text(), "去");
    assert_eq!(level1[0].rank(), 1);
}

#[test]
fn distinct_code_counts_match_char_audit() {
    // 与 generator 单字审计一致:2 码 405 / 3 码 4812 / 4 码 8416 distinct。
    let occupancy = CodeOccupancy::build_baseline_fixed();
    for (length, expected) in [(2usize, 405usize), (3, 4812), (4, 8416)] {
        let distinct = occupancy
            .occupied_codes()
            .filter(|c| c.len() == length)
            .filter(|c| {
                occupancy
                    .group(c)
                    .is_some_and(|g| g.iter().any(|x| x.source().label() == "char_code"))
            })
            .count();
        assert_eq!(distinct, expected, "{length} 码单字 distinct 数");
    }
}

/// 全量硬不变量:每条 PRIMARY 在 current production 中位于 optimizer
/// 给出的 merged rank,使用 generator 的唯一整数权重,且携带真实词频证据。
#[test]
fn primary_layer_reaches_optimizer_merged_ranks() {
    let production = CodeOccupancy::build_current_production();
    let entries = canonical_primary_shortcut_entries();
    assert!(!entries.is_empty(), "PRIMARY 简码层应非空");
    for entry in entries {
        let shortcut = entry.shortcut_code();
        let candidate = production
            .group(shortcut)
            .expect("PRIMARY 简码组存在")
            .iter()
            .find(|candidate| candidate.text() == entry.word())
            .expect("PRIMARY 词必须在其 exact-code 菜单中");
        assert_eq!(candidate.text(), entry.word());
        assert_eq!(candidate.source(), CandidateSource::PrimaryWordShortcut);
        assert_eq!(candidate.rank() as usize, entry.merged_rank());
        assert_eq!(candidate.rime_weight(), entry.rime_weight());
        assert!(
            candidate.frequency_score() > 0,
            "{} PRIMARY 候选必须携带真实词频证据",
            entry.word()
        );
    }
}

/// current production 分层审计:简码层行数等于 canonical TSV 条数；
/// core baseline 保持不变，attested 扩展关系只增加可达候选。
#[test]
fn current_production_layer_audit_counts_shortcuts() {
    let baseline = CodeOccupancy::build_baseline_fixed().layer_audit();
    let production_occupancy = CodeOccupancy::build_current_production();
    assert_eq!(
        production_occupancy.occupied_codes().count(),
        141_138,
        "current production distinct exact code 数"
    );
    let production = production_occupancy.layer_audit();
    let shortcut_count = canonical_primary_shortcut_entries().len();
    let fixed_first_count = canonical_fixed_first_shortcut_entries().len();
    assert_eq!(production.word_shortcut_rows(), shortcut_count);
    assert_eq!(
        production.word_shortcut_2key_rows
            + production.word_shortcut_3key_rows
            + production.word_shortcut_4key_rows
            + production.word_shortcut_5key_rows
            + production.word_shortcut_6key_rows
            + production.word_shortcut_7key_rows,
        shortcut_count
    );
    assert_eq!(production.fixed_first_rows(), fixed_first_count);
    assert_eq!(
        production.fixed_first_3key_rows
            + production.fixed_first_4key_rows
            + production.fixed_first_5key_rows
            + production.fixed_first_6key_rows
            + production.fixed_first_7key_rows,
        fixed_first_count
    );
    assert_eq!(
        production.total_rows(),
        baseline.total_rows() + 2_098 + shortcut_count + fixed_first_count
    );
    // 固定简码层与词层不受扩展字符层影响；字符关系按已审计事实源精确增长。
    assert_eq!(
        production.level1_shortcut_rows,
        baseline.level1_shortcut_rows
    );
    assert_eq!(production.char_2key_rows, 9_254);
    assert_eq!(production.char_3key_rows, 9_724);
    assert_eq!(production.char_4key_rows, 9_873);
    assert_eq!(production.char_2key_rows - baseline.char_2key_rows, 681);
    assert_eq!(production.char_3key_rows - baseline.char_3key_rows, 702);
    assert_eq!(production.char_4key_rows - baseline.char_4key_rows, 715);
    assert_eq!(production.word_4key_rows, baseline.word_4key_rows);
    assert_eq!(production.word_6key_rows, baseline.word_6key_rows);
    assert_eq!(production.word_8key_rows, baseline.word_8key_rows);
}

/// 全量硬不变量:每条 FIXED_FIRST 在统一 static 菜单中严格 rank 1;
/// baseline 与同码 PRIMARY 候选继续按 merged ranking 保持确定次序；
/// attested 扩展字符只允许追加在旧 static 菜单之后。
#[test]
fn fixed_first_layer_is_rank_one_in_merged_menu() {
    let baseline = CodeOccupancy::build_baseline_fixed();
    let production = CodeOccupancy::build_current_production();
    let entries = canonical_fixed_first_shortcut_entries();
    assert!(!entries.is_empty(), "FIXED_FIRST 简码层应非空");
    for entry in entries {
        let shortcut = entry.shortcut_code();
        let baseline_group = baseline.group(shortcut).expect("FF 码在 baseline 有组");
        let fanout = baseline_group.len();
        assert!(
            fanout > 0,
            "{} {} baseline fanout 应 > 0",
            entry.word(),
            shortcut
        );
        assert!(
            baseline_group
                .iter()
                .all(|c| c.source() != CandidateSource::FixedFirstWordShortcut),
            "{shortcut} baseline 组不得含 FIXED_FIRST source"
        );
        let primary_on_code = canonical_primary_shortcut_entries()
            .iter()
            .filter(|primary| primary.shortcut_code() == shortcut)
            .count();
        let group = production.group(shortcut).expect("FF 码在 current 有组");
        let legacy_fanout = fanout + primary_on_code + 1;
        assert!(
            group.len() >= legacy_fanout,
            "{shortcut} current fanout 必须覆盖 baseline + PRIMARY + FIXED_FIRST"
        );
        assert!(
            group[legacy_fanout..]
                .iter()
                .all(|candidate| candidate.source() == CandidateSource::CharCode),
            "{shortcut} 旧 static 菜单之后只允许追加 attested 字符候选"
        );
        let fixed = group
            .iter()
            .find(|candidate| candidate.text() == entry.word())
            .expect("FIXED_FIRST 词必须在其 exact-code 菜单中");
        assert_eq!(fixed.source(), CandidateSource::FixedFirstWordShortcut);
        assert_eq!(fixed.rank(), 1);
        assert_eq!(fixed.rime_weight(), entry.rime_weight());
        assert!(
            fixed.frequency_score() > 0,
            "{} FF 候选必须携带真实词频证据",
            entry.word()
        );
        assert_eq!(
            production.collision_class(shortcut),
            CollisionClass::Multiple,
            "{shortcut} current 碰撞分类应为 MULTIPLE(baseline + FIXED_FIRST)"
        );
    }
}
