//! 码位占用测试:分层行数回归锚点、5/7 键空闲、碰撞分类、扇出哨兵。

use xhup_analyzer::occupancy::{CodeOccupancy, CollisionClass};
use xhup_core::KeySequence;

fn code(text: &str) -> KeySequence {
    text.parse().expect("合法码")
}

#[test]
fn layer_rows_match_production_audit() {
    // 回归锚点:与当前 production 编译表行数审计一致(见 PR #20 审计)。
    let audit = CodeOccupancy::build().layer_audit();
    assert_eq!(audit.level1_shortcut_rows, 26);
    assert_eq!(audit.char_2key_rows, 8573);
    assert_eq!(audit.char_3key_rows, 9022);
    assert_eq!(audit.char_4key_rows, 9158);
    assert_eq!(audit.word_4key_rows, 50000);
    assert_eq!(audit.word_6key_rows, 30000);
    assert_eq!(audit.word_8key_rows, 20000);
    assert_eq!(audit.total_rows(), 126779);
}

#[test]
fn five_and_seven_key_spaces_are_currently_empty() {
    let occupancy = CodeOccupancy::build();
    let stats = occupancy.length_stats();
    assert_eq!(stats.len(), 8, "统计覆盖 1..=8 键");
    assert_eq!(stats[4].length(), 5);
    assert_eq!(stats[4].rows(), 0, "5-key 当前应为空闲空间");
    assert_eq!(stats[4].distinct_codes(), 0);
    assert_eq!(stats[6].length(), 7);
    assert_eq!(stats[6].rows(), 0, "7-key 当前应为空闲空间");
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
    let occupancy = CodeOccupancy::build();
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
    let occupancy = CodeOccupancy::build();
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
    let occupancy = CodeOccupancy::build();
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
