//! 本地用户自适应模型的契约测试(Issue #83 §25 第 9 步)。
//!
//! 断言的是**安全与确定性契约**,不是具体加分数值:
//! 有界、非负、确定性、可重置、损坏安全回退、不泄露用户文本。

use xhup_analyzer::user_model::{
    MAX_BOOST_Q10, USER_MODEL_SCHEMA, USER_MODEL_VERSION, UserModel, UserModelLoad,
};

#[test]
fn boost_is_monotonic_bounded_and_non_negative() {
    let mut model = UserModel::new();
    let mut previous = 0;
    for i in 0..200 {
        model.observe("我们", i as u64);
        let boost = model.boost_q10("我们", i as u64);
        assert!(boost >= previous, "加分必须单调不减(第 {i} 次)");
        assert!(boost >= 0, "加分永不为负(否则会降低可达性)");
        assert!(
            boost <= MAX_BOOST_Q10,
            "加分必须封顶于 MAX_BOOST_Q10,实际 {boost}"
        );
        previous = boost;
    }
    assert_eq!(previous, MAX_BOOST_Q10, "足够多次观测后应达到上限");
}

#[test]
fn unknown_word_has_zero_boost() {
    let mut model = UserModel::new();
    model.observe("我们", 1);
    assert_eq!(model.boost_q10("他们", 1), 0, "未观测词不得获得加分");
    assert_eq!(UserModel::new().boost_q10("我们", 1), 0, "空模型不得加分");
}

#[test]
fn recency_does_not_reduce_boost_below_base() {
    // 最近性只在窗口内加成,过窗口不加分但**绝不惩罚**。
    let mut model = UserModel::new();
    model.observe("我们", 1);
    let fresh = model.boost_q10("我们", 1);
    let stale = model.boost_q10("我们", 100_000);
    assert!(
        fresh >= stale,
        "新鲜观测的加分不得低于陈旧观测({fresh} vs {stale})"
    );
    assert!(stale > 0, "过窗口仍保留基础加分,不得归零或变负");
}

#[test]
fn observation_never_lowers_recorded_recency() {
    // 乱序输入不得让信号抖动回退。
    let mut model = UserModel::new();
    model.observe("我们", 100);
    model.observe("我们", 5);
    assert_eq!(model.signal("我们").unwrap().last_seq(), 100);
    assert_eq!(model.signal("我们").unwrap().selections(), 2);
}

#[test]
fn empty_word_is_ignored() {
    let mut model = UserModel::new();
    model.observe("", 1);
    assert!(model.is_empty());
}

#[test]
fn tsv_round_trip_is_byte_stable_and_deterministic() {
    let mut model = UserModel::new();
    // 乱序插入,序列化必须按词形字典序。
    for (word, seq) in [("我们", 3u64), ("他们", 1), ("你们", 2)] {
        model.observe(word, seq);
    }
    let tsv = model.to_tsv();
    assert!(tsv.starts_with(&format!("# {USER_MODEL_SCHEMA} version=")));
    let restored = UserModel::from_tsv(&tsv).expect("自产 TSV 必须可解析");
    assert_eq!(restored, model, "往返必须逐项一致");
    assert_eq!(restored.to_tsv(), tsv, "往返必须字节稳定");
    // 同一模型多次序列化必须相同。
    assert_eq!(model.to_tsv(), tsv);
}

#[test]
fn load_rejects_future_version_and_falls_back_to_empty() {
    let text = format!(
        "# {USER_MODEL_SCHEMA} version=99 words=1\nword\tselections\tlast_seq\n我们\t1\t1\n"
    );
    let (model, reason) = UserModel::load_or_default(&text);
    assert!(model.is_empty(), "未来版本必须回退空模型,不得部分加载");
    assert_eq!(reason, UserModelLoad::FutureVersion { found: 99 });
    assert!(reason.is_degraded());
}

#[test]
fn load_rejects_wrong_schema() {
    let (model, reason) = UserModel::load_or_default("# some-other-schema/v9\n");
    assert!(model.is_empty());
    assert_eq!(reason, UserModelLoad::SchemaMismatch);
}

#[test]
fn load_rejects_corrupt_lines_without_partial_data() {
    let cases = [
        // 列数不足
        format!("# {USER_MODEL_SCHEMA} version=1\n我们\t1\n"),
        // 计数非法
        format!("# {USER_MODEL_SCHEMA} version=1\n我们\tabc\t1\n"),
        // 零次选择不是有效信号
        format!("# {USER_MODEL_SCHEMA} version=1\n我们\t0\t1\n"),
        // 词形为空
        format!("# {USER_MODEL_SCHEMA} version=1\n\t1\t1\n"),
        // last_seq 非法
        format!("# {USER_MODEL_SCHEMA} version=1\n我们\t1\txyz\n"),
    ];
    for text in cases {
        let (model, reason) = UserModel::load_or_default(&text);
        assert!(model.is_empty(), "损坏输入必须回退空模型:{text:?}");
        assert!(
            matches!(reason, UserModelLoad::CorruptLine { .. }),
            "应报告行级损坏,实际 {reason:?}"
        );
    }
}

#[test]
fn empty_input_loads_as_empty_model_not_error() {
    // 首次运行没有学习数据是**正常**情况,不是损坏。
    let (model, reason) = UserModel::load_or_default("");
    assert!(model.is_empty());
    assert_eq!(reason, UserModelLoad::Ok);
    assert!(reason.is_ok());
}

#[test]
fn debug_output_never_leaks_user_words() {
    // §5 隐私红线:用户词即用户文本,日志与 Debug 必须脱敏。
    let mut model = UserModel::new();
    model.observe("机密词汇", 1);
    let debug = format!("{model:?}");
    assert!(!debug.contains("机密"), "Debug 不得打印词形:{debug}");
    assert!(debug.contains("words: 1"), "应打印规模便于诊断:{debug}");
}

#[test]
fn reset_returns_to_disabled_state_and_is_serializable() {
    let mut model = UserModel::new();
    model.observe("我们", 1);
    assert!(!model.is_empty());
    model.reset();
    assert!(model.is_empty(), "重置后不得保留任何信号");
    assert_eq!(model.boost_q10("我们", 1), 0);
    // 重置后的模型仍必须可序列化/可解析。
    let restored = UserModel::from_tsv(&model.to_tsv()).expect("空模型 TSV 合法");
    assert_eq!(restored, model);
}

#[test]
fn learned_word_boost_can_flip_ranking_but_stays_bounded() {
    // 与 decoder 集成口径:有界加分可以让常用词上浮,但不得压过强证据。
    // 这里只断言数值契约(实际排序集成由 context_replay 覆盖)。
    let mut model = UserModel::new();
    for i in 0..10 {
        model.observe("景点", i);
    }
    let boost = model.boost_q10("景点", 9);
    assert!(boost > 0, "多次选择应产生正向加分");
    assert!(
        boost <= MAX_BOOST_Q10,
        "即使 10 次选择也不得超过上限(否则会盖过静态证据)"
    );
    // 上限对应的语义:最多相当于两次词频翻倍。
    assert_eq!(MAX_BOOST_Q10, 2048);
    assert_eq!(USER_MODEL_VERSION, 1);
}
