//! KdconvBigramScorer 与真实入库 bigram 数据的一致性测试。

use std::num::NonZeroUsize;

use xhup_core::KeySequence;
use xhup_decoder::{
    BigramModel, CandidateKind, EdgeCandidate, KdconvBigramScorer, Lattice, RuntimeContext, Span,
    rank_paths,
};

const BIGRAM_TSV: &str = include_str!("../../../data/corpus/kdconv_bigram.tsv");

fn complete_paths(lattice: &Lattice, limit: usize) -> xhup_decoder::PathSet {
    lattice.complete_paths(NonZeroUsize::new(limit).unwrap())
}

#[test]
fn real_bigram_data_parses_and_is_deterministic() {
    let model = BigramModel::from_tsv(BIGRAM_TSV).expect("入库 bigram TSV 必须可解析");
    assert!(model.pair_count() > 200_000, "转移对应覆盖全量语料");
    assert!(model.vocabulary_size() > 20_000);
    // 高频会话转移应存在(来自 KdConv 高频对话词)。
    assert!(model.transition_count("<s>", "是的") >= 3885);
    assert!(model.transition_count("这个", "景点") >= 2019);
    assert!(model.unigram_count("什么") > 0);
}

#[test]
fn empty_context_matches_baseline_ranking() {
    let model = BigramModel::from_tsv(BIGRAM_TSV).expect("可解析");
    let scorer = KdconvBigramScorer::new(model);
    let baseline = xhup_decoder::BaselineScorer::default();

    let mut lattice = Lattice::new("yjjqugmk".parse::<KeySequence>().unwrap());
    for (text, start, end, frequency) in [
        ("研究", 0, 4, 266_843_u64),
        ("生命", 4, 8, 80_039),
        ("研究生", 0, 6, 72_173),
        ("命", 6, 8, 6_600),
    ] {
        lattice
            .add_edge(
                Span::new(start, end).unwrap(),
                EdgeCandidate::new(text, CandidateKind::HotWord, frequency).unwrap(),
            )
            .unwrap();
    }
    let context = RuntimeContext::new("", "yjjqugmk".parse().unwrap());
    let paths = complete_paths(&lattice, 32);
    let with_bigram = rank_paths(&scorer, &context, &lattice, paths.paths());
    let paths = complete_paths(&lattice, 32);
    let with_baseline = rank_paths(&baseline, &context, &lattice, paths.paths());
    assert_eq!(
        with_bigram[0].segments(),
        with_baseline[0].segments(),
        "无 committed context 时 bigram scorer 不应改变排序"
    );
}

#[test]
fn committed_context_uses_real_transition_evidence() {
    let model = BigramModel::from_tsv(BIGRAM_TSV).expect("可解析");
    let scorer = KdconvBigramScorer::new(model);

    let mut lattice = Lattice::new("yjjqugmk".parse::<KeySequence>().unwrap());
    for (text, start, end, frequency) in [
        ("研究", 0, 4, 266_843_u64),
        ("生命", 4, 8, 80_039),
        ("研究生", 0, 6, 72_173),
        ("命", 6, 8, 6_600),
    ] {
        lattice
            .add_edge(
                Span::new(start, end).unwrap(),
                EdgeCandidate::new(text, CandidateKind::HotWord, frequency).unwrap(),
            )
            .unwrap();
    }

    // 真实语料中「教育→研究生」(3 次)与「的→生命」(23 次)都有转移证据;
    // 教育|研究生:上下文尾词「教育」为路径首段提供真实奖励,命中的是
    // 研究生|命 路径的首段 → 首段奖励差异决定排序是否翻转。
    let edu_grad = BIGRAM_TSV.contains("教育\t研究生\t");
    let de_life = BIGRAM_TSV.contains("的\t生命\t23\n");
    assert!(edu_grad && de_life, "真实转移证据应存在");

    // 教育 → 研究生|命:首段奖励;的 → 研究|生命:首段奖励。两者都有证据时
    // 排序由 (baseline + 首段奖励) 决定。
    //
    // 标定后的语义(transition_weight = 2):极弱证据**不应**翻转约 33 倍的
    // 词频差。实测 log2_q10(3) × 2 = 4096 < 词频差 5659,故「教育」(3 次
    // 共现)不足以把「研究生|命」翻到首位 —— 这是**正确**行为,而非回归:
    // 旧的 transition_weight=256 会让 3 次共现压过 5659 Q10 的词频证据。
    // 本测试因此断言的是「奖励被真实计入 + 不足以翻转过强词频差」。
    let context = RuntimeContext::new("教育", "yjjqugmk".parse().unwrap());
    let paths = complete_paths(&lattice, 32);
    let ranked = rank_paths(&scorer, &context, &lattice, paths.paths());
    // 上下文证据必须被真实计入(breakdown 非零)。
    assert!(
        ranked
            .iter()
            .any(|path| path.breakdown().transition_reward > 0),
        "真实转移证据必须体现在 breakdown 中"
    );
    // 3 次共现在标定权重下不足以翻转过强的词频差。
    assert_eq!(
        ranked[0].segments().join("|"),
        "研究|生命",
        "弱证据不得压过数量级更高的词频证据"
    );
}
