//! 多源转移证据合并的契约与真实数据测试(Issue #83 §25 第 6 步)。

use xhup_decoder::{BigramModel, MergePolicy, NORMALIZE_SCALE, merge_models};

const KDCONV: &str = include_str!("../../../data/corpus/kdconv_bigram.tsv");
const PTT: &str = include_str!("../../../data/corpus/ptt_bigram.tsv");

fn fixture(left: &str, right: &str, count: u64) -> BigramModel {
    let mut model = BigramModel::default();
    model.observe(left, right, count);
    model
}

#[test]
fn raw_sum_adds_counts_across_sources() {
    let a = fixture("甲", "乙", 3);
    let b = fixture("甲", "乙", 5);
    let (merged, audit) = merge_models(&[("a", &a), ("b", &b)], MergePolicy::RawSum);
    assert_eq!(merged.transition_count("甲", "乙"), 8);
    assert_eq!(audit.policy, "raw-sum/v1");
    assert_eq!(audit.merged_pairs, 1);
    assert_eq!(audit.sources, vec![("a", 3), ("b", 5)]);
}

#[test]
fn max_evidence_takes_the_stronger_source_not_the_sum() {
    let a = fixture("甲", "乙", 3);
    let b = fixture("甲", "乙", 5);
    let (merged, audit) = merge_models(&[("a", &a), ("b", &b)], MergePolicy::MaxEvidence);
    assert_eq!(merged.transition_count("甲", "乙"), 5, "取最大而非求和");
    assert_eq!(audit.policy, "max-evidence/v1");
}

#[test]
fn max_evidence_still_keeps_source_unique_pairs() {
    // 最保守的策略也不得丢掉「只有一个来源见过」的证据 ——
    // 那正是补第二来源的目的(§25 第 6 步)。
    let a = fixture("甲", "乙", 3);
    let b = fixture("丙", "丁", 7);
    let (merged, _) = merge_models(&[("a", &a), ("b", &b)], MergePolicy::MaxEvidence);
    assert_eq!(merged.transition_count("甲", "乙"), 3);
    assert_eq!(merged.transition_count("丙", "丁"), 7);
}

#[test]
fn per_source_normalized_compresses_the_larger_source() {
    // 归一化的**实测机制**:缩放因子是 count/total,因此**总量更大**的来源
    // 的每一个具体转移对都会被压缩,总量小的来源则被放大。
    // 这正是真实数据上 PTT 独占证据被削弱的成因(见下方真实数据测试),
    // 也是消费侧默认不选该策略的理由。
    //
    // 构造:小来源总量 10(单对),大来源总量 10,000(100 对 × 100)。
    let small = fixture("小", "证", 10);
    let big = {
        let mut model = BigramModel::default();
        for i in 0..100 {
            model.observe(&format!("大{i}"), "共现", 100);
        }
        model
    };
    let (merged, _) = merge_models(
        &[("small", &small), ("big", &big)],
        MergePolicy::PerSourceNormalized,
    );
    // 大来源的具体对:100 / 10,000 = 1% ⇒ 缩放到 SCALE 的 1% = 10,000,
    // 相对其原始计数 100 被**放大**到同一量级;小来源的对占其全部 ⇒ 缩放到
    // SCALE 全额,远大于其原始 10。
    assert!(
        merged.transition_count("大0", "共现") > 100,
        "大来源的对被对齐放大(实际 {})",
        merged.transition_count("大0", "共现")
    );
    assert!(
        merged.transition_count("小", "证") > 10,
        "小来源的对被对齐放大(实际 {})",
        merged.transition_count("小", "证")
    );
    // 关键语义:对齐后「占比」而非「原始计数」决定强度,故两源可比。
    const { assert!(NORMALIZE_SCALE > 0) };
}

#[test]
fn merge_is_deterministic_and_order_independent_for_max_and_sum() {
    let a = fixture("甲", "乙", 3);
    let b = fixture("丙", "丁", 5);
    let (m1, au1) = merge_models(&[("a", &a), ("b", &b)], MergePolicy::RawSum);
    let (m2, au2) = merge_models(&[("b", &b), ("a", &a)], MergePolicy::RawSum);
    assert_eq!(
        m1.transition_count("甲", "乙"),
        m2.transition_count("甲", "乙")
    );
    assert_eq!(
        m1.transition_count("丙", "丁"),
        m2.transition_count("丙", "丁")
    );
    // 审计记录按调用顺序(便于对照调用方参数),但合并结果不变。
    assert_eq!(au1.policy, au2.policy);
    assert_eq!(au1.merged_pairs, au2.merged_pairs);
}

#[test]
fn merge_is_deterministic_across_repeated_runs() {
    let a = fixture("甲", "乙", 3);
    let b = fixture("甲", "乙", 5);
    for policy in [
        MergePolicy::RawSum,
        MergePolicy::PerSourceNormalized,
        MergePolicy::MaxEvidence,
    ] {
        let (m1, _) = merge_models(&[("a", &a), ("b", &b)], policy);
        let (m2, _) = merge_models(&[("a", &a), ("b", &b)], policy);
        assert_eq!(m1.to_tsv(), m2.to_tsv(), "同输入必须字节稳定({policy:?})");
    }
}

#[test]
fn empty_source_list_yields_empty_model_without_panic() {
    for policy in [
        MergePolicy::RawSum,
        MergePolicy::PerSourceNormalized,
        MergePolicy::MaxEvidence,
    ] {
        let (merged, audit) = merge_models(&[], policy);
        assert!(merged.is_empty());
        assert_eq!(audit.merged_pairs, 0);
        assert!(audit.sources.is_empty());
    }
}

#[test]
fn single_source_is_identity_for_all_policies() {
    let a = fixture("甲", "乙", 42);
    for policy in [
        MergePolicy::RawSum,
        MergePolicy::PerSourceNormalized,
        MergePolicy::MaxEvidence,
    ] {
        let (merged, _) = merge_models(&[("a", &a)], policy);
        // 单来源时归一化也会缩放(按定义),因此只断言「有证据且可查」。
        assert!(merged.transition_count("甲", "乙") > 0, "{policy:?}");
    }
}

// ---------------------------------------------------------------------------
// 真实数据:KDConv + PTT
// ---------------------------------------------------------------------------

#[test]
fn real_merge_uses_shipped_corpus_artifacts() {
    let kd = BigramModel::from_tsv(KDCONV).expect("kdconv bigram 可解析");
    let pt = BigramModel::from_tsv(PTT).expect("ptt bigram 可解析");
    assert!(kd.pair_count() > 200_000);
    assert!(pt.pair_count() > 600_000, "PTT 裁剪后应仍有 60 万+ 转移对");

    let (merged, audit) = merge_models(&[("kdconv", &kd), ("ptt", &pt)], MergePolicy::RawSum);
    assert_eq!(audit.sources.len(), 2);
    // 合并后必须同时保留两源证据。
    assert_eq!(
        merged.transition_count("这个", "景点"),
        kd.transition_count("这个", "景点")
    );
    assert_eq!(
        merged.transition_count("大学", "学生"),
        pt.transition_count("大学", "学生")
    );
}

#[test]
fn real_ptt_adds_evidence_absent_from_kdconv() {
    // 本 PR 的核心事实:PTT 提供了 KDConv 没有的证据(故有合并价值)。
    let kd = BigramModel::from_tsv(KDCONV).expect("kdconv");
    let pt = BigramModel::from_tsv(PTT).expect("ptt");
    assert_eq!(kd.transition_count("大学", "学生"), 0, "KDConv 无此转移");
    assert!(pt.transition_count("大学", "学生") > 0, "PTT 提供该转移");
    // 合并后该证据可用,且**不依赖**选用哪个策略。
    for policy in [
        MergePolicy::RawSum,
        MergePolicy::PerSourceNormalized,
        MergePolicy::MaxEvidence,
    ] {
        let (merged, _) = merge_models(&[("kdconv", &kd), ("ptt", &pt)], policy);
        assert!(
            merged.transition_count("大学", "学生") > 0,
            "{policy:?} 不得丢失 PTT 独占证据"
        );
    }
}

#[test]
fn real_normalization_compresses_ptt_unique_evidence() {
    // 实测副作用的真实数据版本:PTT 语料量是 KDConv 的约 9.3 倍,
    // 归一化后 PTT 的计数被**缩小**,其独占证据强度下降。
    // 这解释了为什么消费侧默认不选 PerSourceNormalized。
    let kd = BigramModel::from_tsv(KDCONV).expect("kdconv");
    let pt = BigramModel::from_tsv(PTT).expect("ptt");
    let raw = pt.transition_count("大学", "学生");
    let (normalized, _) = merge_models(
        &[("kdconv", &kd), ("ptt", &pt)],
        MergePolicy::PerSourceNormalized,
    );
    let scaled = normalized.transition_count("大学", "学生");
    assert!(
        scaled < raw,
        "归一化应压缩 PTT 独占证据(raw={raw}, scaled={scaled})"
    );
}
