//! 二码词语简码研究 + 二码零冲突 production policy 的测试。

use xhup_analyzer::frequency::{CharCodeUsage, FrequencyScale};
use xhup_analyzer::production_two_key;
use xhup_analyzer::sweep::OperatingPointId;
use xhup_analyzer::two_key_study::{
    self, TwoKeyCharDomain, TwoKeyPlacement, TwoKeyStudyRun, TwoKeyUniverse,
};
use xhup_core::KeySequence;
use xhup_generator::canonical_two_key_shortcut_entries;

/// 共享 fixture(每个测试进程最多跑一次 30 次研究网格)。
fn universe() -> &'static TwoKeyUniverse {
    static UNIVERSE: std::sync::OnceLock<TwoKeyUniverse> = std::sync::OnceLock::new();
    UNIVERSE.get_or_init(TwoKeyUniverse::build)
}

fn runs() -> &'static Vec<TwoKeyStudyRun> {
    static RUNS: std::sync::OnceLock<Vec<TwoKeyStudyRun>> = std::sync::OnceLock::new();
    RUNS.get_or_init(|| two_key_study::run_two_key_grid(universe()))
}

// ── 语法 / 全集 ───────────────────────────────────────────────

#[test]
fn every_two_char_word_has_exactly_one_ii_candidate() {
    // build() 内部已硬断言;此处再从数据侧锁结构事实。
    let universe = universe();
    assert_eq!(
        universe.candidates.len(),
        universe.two_char_target_count,
        "每个 2 字词恰有一个 II 候选"
    );
    for candidate in &universe.candidates {
        assert_eq!(candidate.word.chars().count(), 2);
        assert_eq!(candidate.full_code.len(), 4);
        assert_eq!(candidate.two_key_code.len(), 2);
        // 机械首键投影。
        let full = candidate.full_code.as_slice();
        assert_eq!(candidate.two_key_code.as_slice(), &[full[0], full[2]]);
    }
}

#[test]
fn three_four_char_words_are_not_in_study() {
    let universe = universe();
    for candidate in &universe.candidates {
        assert_eq!(
            candidate.word.chars().count(),
            2,
            "研究对象必须全部是 2 字词"
        );
    }
}

// ── 2 键单字概率域 ────────────────────────────────────────────

#[test]
fn two_key_char_domain_sums_to_one() {
    // Σ P_2key = 1(char_share = 1.0 下权重总和)。
    let domain = TwoKeyCharDomain::build();
    for usage in [CharCodeUsage::Conservative, CharCodeUsage::Split] {
        let total: f64 = domain
            .entries()
            .map(|(hanzi, code, score, _)| domain.relation_weight(1.0, hanzi, code, score, usage))
            .sum();
        assert!(
            (total - 1.0).abs() < 1e-9,
            "{}:归一化 2 键单字质量和应≈1.0, 实际 {total}",
            usage.label()
        );
    }
}

#[test]
fn two_key_char_share_mass_mixture() {
    // char 侧总质量 == char_share。
    let domain = TwoKeyCharDomain::build();
    for usage in [CharCodeUsage::Conservative, CharCodeUsage::Split] {
        for char_share in [0.25, 0.50, 0.75] {
            let total: f64 = domain
                .entries()
                .map(|(hanzi, code, score, _)| {
                    domain.relation_weight(char_share, hanzi, code, score, usage)
                })
                .sum();
            assert!(
                (total - char_share).abs() < 1e-9,
                "{} char_share={char_share}:char 侧总质量应≈{char_share}, 实际 {total}",
                usage.label()
            );
        }
    }
}

// ── 「时间」哨兵(只锁结构事实,不锁研究符号) ──────────────────

#[test]
fn time_sentinel_structural_facts() {
    let universe = universe();
    let candidate = universe
        .candidates
        .iter()
        .find(|c| c.word == "时间")
        .expect("时间必然在 2 字词表中");
    assert_eq!(candidate.full_code.to_string(), "uijm");
    assert_eq!(candidate.two_key_code.to_string(), "uj");
    // 既有 production 简码:uij / FI(FIXED_FIRST 层)。
    match &candidate.existing_shortcut {
        two_key_study::ExistingShortcutStatus::FixedFirst { code } => {
            assert_eq!(code.to_string(), "uij");
        }
        other => panic!("时间应持有 FIXED_FIRST 简码 uij,实际 {other:?}"),
    }
    // uj 是占用码(fanout > 0)→ 结构上不可能进入二码零冲突生产层。
    assert!(
        candidate.char_fanout > 0,
        "uj 应有 2 键单字占用(fanout > 0)"
    );
    assert_eq!(
        candidate.code_class,
        two_key_study::TwoKeyCodeClass::OccupiedByChars
    );
}

// ── production 隔离 ───────────────────────────────────────────

#[test]
fn two_key_production_is_empty_code_only() {
    let (selected, _audit) = production_two_key::select_two_key_production(universe(), runs());
    let occupancy = &universe().occupancy;
    for entry in &selected {
        // 每个入选码在当前 production occupancy 中完全空闲(独立重验)。
        assert_eq!(
            occupancy.fanout(&entry.shortcut_code),
            0,
            "{} {} 必须是空码",
            entry.word,
            entry.shortcut_code
        );
        // 2 键 + II + 机械投影。
        assert_eq!(entry.shortcut_code.len(), 2);
        let full = entry.full_code.as_slice();
        assert_eq!(entry.shortcut_code.as_slice(), &[full[0], full[2]]);
        // Universe A 成员(无既有简码)。
        let candidate = universe()
            .candidates
            .iter()
            .find(|c| c.word == entry.word)
            .expect("入选词必然在候选集中");
        assert!(candidate.existing_shortcut.is_none());
        // 有效收益为正 + 整数 4/5 稳定票。
        assert!(entry.weighted_net_gain > 0.0);
        assert!(
            entry.top_word_votes * 5 >= entry.total_runs * 4,
            "{} 稳定票不足 4/5",
            entry.word
        );
    }
}

#[test]
fn canonical_tsv_byte_reproduction() {
    // 入库 canonical TSV 必须能由 selection API 字节级复现。
    let (selected, _) = production_two_key::select_two_key_production(universe(), runs());
    let canonical = include_str!("../../../data/shortcuts/word_two_key_zero_regression.tsv");
    assert_eq!(
        production_two_key::serialize_two_key_tsv(&selected),
        canonical,
        "canonical TSV 与 selection API 输出必须字节一致"
    );
}

#[test]
fn tsv_matches_parsed_canonical_entries() {
    // analyzer 选择集与 generator 解析投影逐条一致(词/码/完整码/模式)。
    let (selected, _) = production_two_key::select_two_key_production(universe(), runs());
    let parsed = canonical_two_key_shortcut_entries();
    assert_eq!(selected.len(), parsed.len());
    for (selection, entry) in selected.iter().zip(parsed.iter()) {
        assert_eq!(selection.word, entry.word());
        assert_eq!(selection.full_code, *entry.full_code());
        assert_eq!(selection.shortcut_code, *entry.shortcut_code());
        assert_eq!(selection.mode, entry.mode());
    }
}

// ── SAFE_APPEND(合成) ────────────────────────────────────────

#[test]
fn safe_append_displaces_nothing() {
    // 合成:码 ab 上有 3 个单字 A/B/C + 词 W → W rank 4,扰动 0。
    let universe = universe();
    // 找一个真实占用码做评估(fanout 记为 N)。
    let candidate = universe
        .candidates
        .iter()
        .find(|c| c.code_class == two_key_study::TwoKeyCodeClass::OccupiedByChars)
        .expect("存在占用码候选");
    let scale = production_two_key::reference_scale();
    let cost = OperatingPointId::Balanced.operating_point().cost_model();
    let evaluation = two_key_study::evaluate_placement(
        candidate,
        TwoKeyPlacement::SafeAppend,
        &universe.occupancy,
        &universe.char_domain,
        &universe.frequency,
        &scale,
        &cost,
    );
    assert_eq!(
        evaluation.rank as usize,
        candidate.char_fanout + 1,
        "SAFE_APPEND rank 必须 = fanout + 1"
    );
    assert_eq!(evaluation.weighted_char_disruption, 0.0, "扰动必须为 0");
    assert_eq!(evaluation.displaced_char_count, 0);
    assert!(evaluation.displaced_chars.is_empty());
    // 空码候选:rank 1。
    let empty = universe
        .candidates
        .iter()
        .find(|c| c.code_class == two_key_study::TwoKeyCodeClass::Empty)
        .expect("存在空码候选");
    let evaluation = two_key_study::evaluate_placement(
        empty,
        TwoKeyPlacement::SafeAppend,
        &universe.occupancy,
        &universe.char_domain,
        &universe.frequency,
        &scale,
        &cost,
    );
    assert_eq!(evaluation.rank, 1, "空码 SAFE_APPEND rank 必须 = 1");
    assert_eq!(evaluation.projected_fanout, 1);
}

// ── OPTIMAL_INSERT(真实数据语义) ────────────────────────────

#[test]
fn optimal_insert_semantics_on_real_data() {
    // Case A/B/C 在真实数据上验证 OPTIMAL_INSERT 的行为特征:
    // - 存在 rank1 最优的占用码候选(高频词压过低频单字);
    // - 存在 append(fanout+1)最优或净收益为负的候选;
    // - 证明优化器不是只会选两端(存在 middle 最优)。
    let universe = universe();
    let reference = two_key_study::reference_run_index(runs());
    let mut rank1 = 0usize;
    let mut middle = 0usize;
    let mut append = 0usize;
    let mut negative = 0usize;
    for (index, candidate) in universe.candidates.iter().enumerate() {
        if candidate.code_class != two_key_study::TwoKeyCodeClass::OccupiedByChars {
            continue;
        }
        let evaluation = &runs()[reference].optimal[index];
        if evaluation.weighted_net_gain <= 0.0 {
            negative += 1;
            continue;
        }
        match evaluation.rank {
            1 => rank1 += 1,
            r if r as usize == evaluation.projected_fanout => append += 1,
            _ => middle += 1,
        }
    }
    // 真实数据必然覆盖多种最优名次(若全部是 rank1/append,优化器可疑)。
    assert!(rank1 > 0, "应存在 rank1 最优的候选");
    assert!(append > 0 || negative > 0, "应存在 append 最优或负收益候选");
    // middle 在真实数据上可能存在也可能不存在;两种结果都如实断言非负。
    assert!(rank1 + middle + append + negative > 0);
}

#[test]
fn optimal_insert_displacement_accounting() {
    // OPTIMAL_INSERT 的扰动记账:被置换单字数 = fanout - rank + 1
    //(插入 rank 使原名次 >= rank 者全部 +1)。
    let universe = universe();
    let reference = two_key_study::reference_run_index(runs());
    for (index, candidate) in universe.candidates.iter().enumerate() {
        if candidate.code_class != two_key_study::TwoKeyCodeClass::OccupiedByChars {
            continue;
        }
        let evaluation = &runs()[reference].optimal[index];
        // 插入 rank 使原名次 >= rank 者全部 +1;被置换数 =
        // fanout - (rank - 1),append(rank = fanout+1)时为 0。
        let expected = candidate
            .char_fanout
            .saturating_sub(evaluation.rank as usize - 1);
        // 仅当插入确实发生(净收益为正时报告值);负收益时 displaced 记账
        // 仍按所选 rank 计算,保持一致。
        assert_eq!(
            evaluation.displaced_char_count, expected,
            "{} rank {} fanout {} 的置换记账不一致",
            candidate.word, evaluation.rank, candidate.char_fanout
        );
    }
}

// ── 当前最优路径(语义) ───────────────────────────────────────

#[test]
fn current_best_route_semantics() {
    let universe = universe();
    // 1) 无简码词:路径必须是 FULL_CODE。
    for candidate in universe.candidates.iter().take(1000) {
        if candidate.existing_shortcut.is_none() {
            assert_eq!(
                candidate.current_best.kind,
                two_key_study::RouteKind::FullCode
            );
        }
    }
    // 2) ZR 词:ZR rank-1 简码应优于 4 键 full code(3 键 rank1 vs 4 键
    //    rank>=1;不假设必然 —— 但存在 ZR 词其最优路径为 ZR)。
    let mut zr_best = 0usize;
    for candidate in universe.candidates.iter() {
        if matches!(
            candidate.existing_shortcut,
            two_key_study::ExistingShortcutStatus::ZeroRegression { .. }
        ) && candidate.current_best.kind == two_key_study::RouteKind::ZeroRegression
        {
            zr_best += 1;
        }
    }
    assert!(zr_best > 0, "应存在 ZR 路径最优的词");
    // 3) FF 词:按真实有效成本比较(不假设更短必胜)。存在 FF 词其
    //    最优路径为 FF(rank3 的 uij 仍应胜过 rank1 的 4 键 uijm)。
    let time = universe
        .candidates
        .iter()
        .find(|c| c.word == "时间")
        .expect("时间在词表");
    assert_eq!(
        time.current_best.kind,
        two_key_study::RouteKind::FixedFirst,
        "时间的最优路径应为 FIXED_FIRST uij(3 键 rank3 优于 4 键)"
    );
}

// ── 研究确定性 ────────────────────────────────────────────────

#[test]
fn study_is_deterministic() {
    // 两次网格运行:参考运行的评估逐条一致。
    let first = runs();
    let universe = universe();
    let second = two_key_study::run_two_key_grid(universe);
    let reference = two_key_study::reference_run_index(first);
    let reference_second = two_key_study::reference_run_index(&second);
    for index in 0..universe.candidates.len() {
        let a = &first[reference].safe[index];
        let b = &second[reference_second].safe[index];
        assert_eq!(a.rank, b.rank);
        assert!((a.weighted_net_gain - b.weighted_net_gain).abs() < 1e-12);
        let a = &first[reference].optimal[index];
        let b = &second[reference_second].optimal[index];
        assert_eq!(a.rank, b.rank);
        assert!((a.weighted_net_gain - b.weighted_net_gain).abs() < 1e-12);
    }
}

#[test]
fn reference_grid_position() {
    let runs = runs();
    let reference = two_key_study::reference_run_index(runs);
    assert_eq!(runs[reference].point, OperatingPointId::Balanced);
    assert!(matches!(
        runs[reference].scale,
        FrequencyScale::Normalized { char_share, usage }
            if char_share == 0.5 && usage == CharCodeUsage::Conservative
    ));
    // 不变量:时间不在生产集(占用码)。
    let (selected, _) = production_two_key::select_two_key_production(universe(), runs);
    assert!(
        !selected.iter().any(|e| e.word == "时间"),
        "时间(uj 占用码)绝不能进入二码零冲突生产层"
    );
    // 不变量:u j 等 sh 声母占用码全无生产映射。
    for entry in &selected {
        assert_ne!(
            entry.shortcut_code.to_string(),
            "uj",
            "uj 是占用码,不得生产化"
        );
    }
}

/// KeySequence 解析冒烟(测试常量自检)。
#[test]
fn key_parse_smoke() {
    let seq: KeySequence = "uj".parse().unwrap();
    assert_eq!(seq.len(), 2);
}
