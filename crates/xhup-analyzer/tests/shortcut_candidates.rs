//! 候选枚举测试:两个语法的 时间 哨兵(P0)、候选合法性、确定性、
//! Monotone V2 结构不变量。

use std::collections::BTreeSet;

use xhup_analyzer::candidates::{CandidateEnumerationSpec, Mode, enumerate_targets_with_spec};
use xhup_core::KeySequence;
use xhup_generator::word_code_analysis_entries;

/// 从完整码与模式重投影,验证候选码确实只由 F/I 投影构成。
fn reproject(full_code: &KeySequence, modes: &[Mode]) -> Vec<xhup_core::Key> {
    let mut keys = Vec::new();
    let (chunks, _) = full_code.as_slice().as_chunks::<2>();
    for (index, chunk) in chunks.iter().enumerate() {
        match modes[index] {
            Mode::Full => keys.extend_from_slice(chunk),
            Mode::Initial => keys.push(chunk[0]),
        }
    }
    keys
}

/// Legacy 冻结哨兵:「时间」候选必须恰好是 uij(FI)与 ujm(IF)——
/// 这是 PR #21/#22 冻结 legacy-any-fi-v1 语法的历史行为(II/uj 被冻结的
/// 枚举期 len>=3 过滤移除)。
#[test]
fn legacy_time_sentinel_candidates_are_exactly_fi_and_if() {
    let words = word_code_analysis_entries();
    let (targets, _) =
        enumerate_targets_with_spec(&words, CandidateEnumerationSpec::LEGACY_V1_FROZEN);
    let target = targets
        .iter()
        .find(|t| t.word() == "时间")
        .expect("时间必然在 production 词表中");
    assert_eq!(target.full_code().to_string(), "uijm");
    let candidates: BTreeSet<(String, String)> = target
        .candidates()
        .iter()
        .map(|c| (c.shortcut_code().to_string(), c.mode().pattern()))
        .collect();
    assert_eq!(
        candidates,
        BTreeSet::from([
            ("uij".to_string(), "FI".to_string()),
            ("ujm".to_string(), "IF".to_string())
        ]),
        "Legacy V1 的时间 shortcut 候选必须恰好是 uij(FI) 与 ujm(IF)"
    );
}

/// Monotone V2 哨兵(P0):「时间」理论候选必须恰好是 uij(FI)与 uj(II);
/// ujm(IF)在 monotone 语法下结构性不存在。
#[test]
fn monotone_time_sentinel_candidates_are_exactly_fi_and_ii() {
    let words = word_code_analysis_entries();
    let (targets, stats) =
        enumerate_targets_with_spec(&words, CandidateEnumerationSpec::MONOTONE_V2_THEORETICAL);
    let target = targets
        .iter()
        .find(|t| t.word() == "时间")
        .expect("时间必然在 production 词表中");
    assert_eq!(target.full_code().to_string(), "uijm");
    let candidates: Vec<(String, String)> = target
        .candidates()
        .iter()
        .map(|c| (c.shortcut_code().to_string(), c.mode().pattern()))
        .collect();
    assert_eq!(
        candidates,
        vec![
            ("uij".to_string(), "FI".to_string()),
            ("uj".to_string(), "II".to_string())
        ],
        "Monotone V2 的时间理论候选必须恰好是 uij(FI) 与 uj(II);ujm(IF) 结构性非法"
    );
    // 全宇宙 invariant:Monotone V2 理论候选数 = 去重后候选数 = 每词字数之和,
    // dedup 恒为 0(不同 k 的码长互不相同)。
    assert_eq!(stats.dedup_removed(), 0, "Monotone V2 不得有 dedup");
}

/// Monotone V2 模式不变量(全量):2 字 {FI,II}、3 字 {FFI,FII,III}、
/// 4 字 {FFFI,FFII,FIII,IIII},不得存在任何其它模式(尤其不得含 I…F)。
#[test]
fn monotone_universe_has_only_monotone_suffix_patterns() {
    let words = word_code_analysis_entries();
    let (targets, _) =
        enumerate_targets_with_spec(&words, CandidateEnumerationSpec::MONOTONE_V2_THEORETICAL);
    let legal: [(&str, [&str; 4]); 3] = [
        ("2", ["FI", "II", "", ""]),
        ("3", ["FFI", "FII", "III", ""]),
        ("4", ["FFFI", "FFII", "FIII", "IIII"]),
    ];
    for target in &targets {
        let char_count = target.word().chars().count();
        let expected: BTreeSet<&str> = legal[char_count - 2]
            .1
            .iter()
            .filter(|p| !p.is_empty())
            .copied()
            .collect();
        let actual: BTreeSet<String> = target
            .candidates()
            .iter()
            .map(|c| c.mode().pattern())
            .collect();
        assert_eq!(
            actual,
            expected
                .iter()
                .map(|s| s.to_string())
                .collect::<BTreeSet<_>>(),
            "{} ({} 字) 的 Monotone V2 模式集不合法",
            target.word(),
            char_count
        );
    }
}

/// Monotone V2 理论候选不变量(全量):每词 theoretical == actual == 字数。
#[test]
fn monotone_theoretical_equals_actual_per_word() {
    let words = word_code_analysis_entries();
    let (targets, stats) =
        enumerate_targets_with_spec(&words, CandidateEnumerationSpec::MONOTONE_V2_THEORETICAL);
    let expected_total: usize = targets.iter().map(|t| t.word().chars().count()).sum();
    assert_eq!(stats.theoretical, expected_total);
    assert_eq!(stats.actual, expected_total);
    for target in &targets {
        assert_eq!(
            target.candidates().len(),
            target.word().chars().count(),
            "{} 的理论候选数应恰为字数",
            target.word()
        );
    }
}

/// 候选合法性(两个语法):长度、字符集、重投影、(词, 码) 去重。
#[test]
fn every_candidate_is_a_legal_fi_projection() {
    let words = word_code_analysis_entries();
    for spec in [
        CandidateEnumerationSpec::LEGACY_V1_FROZEN,
        CandidateEnumerationSpec::MONOTONE_V2_THEORETICAL,
    ] {
        let (targets, stats) = enumerate_targets_with_spec(&words, spec);
        assert!(stats.actual > 0);
        let mut candidate_count = 0usize;
        for target in &targets {
            let char_count = target.word().chars().count();
            let full_len = target.full_code().len();
            assert_eq!(full_len, char_count * 2, "词码长度应为字数两倍");
            let mut seen_codes: BTreeSet<&KeySequence> = BTreeSet::new();
            for candidate in target.candidates() {
                candidate_count += 1;
                let code = candidate.shortcut_code();
                // 长度:满足枚举规格最小值,且严格小于完整码
                assert!(
                    code.len() >= spec.min_length,
                    "{} {} 长度 < {}",
                    target.word(),
                    code,
                    spec.min_length
                );
                assert!(
                    code.len() < full_len,
                    "{} {} 不短于完整码",
                    target.word(),
                    code
                );
                // 只含小写 a-z
                assert!(
                    code.to_string().chars().all(|c| c.is_ascii_lowercase()),
                    "{} 含非小写字母",
                    code
                );
                // 模式长度 = 字数,且码与模式逐字 F/I 重投影一致(保持原字序)
                let modes = candidate.mode().modes();
                assert_eq!(modes.len(), char_count);
                assert_eq!(
                    reproject(target.full_code(), modes),
                    code.as_slice(),
                    "{} {} 与模式 {} 的重投影不一致",
                    target.word(),
                    code,
                    candidate.mode().pattern()
                );
                // 模式属于该语法
                assert!(
                    spec.grammar.accepts(candidate.mode()),
                    "{} 模式 {} 不属于语法 {}",
                    target.word(),
                    candidate.mode().pattern(),
                    spec.grammar.label()
                );
                // (词, 码) 去重
                assert!(seen_codes.insert(code), "{} 内码重复", target.word());
            }
            // 每词候选数上界:Legacy 2 字 2/3 字 7/4 字 15;Monotone = 字数。
            let upper = match spec.grammar {
                xhup_analyzer::candidates::CandidateGrammar::LegacyAnyFiV1 => match char_count {
                    2 => 2,
                    3 => 7,
                    4 => 15,
                    other => panic!("词字数应为 2/3/4: {other}"),
                },
                xhup_analyzer::candidates::CandidateGrammar::MonotoneSuffixInitialsV2 => char_count,
            };
            assert!(target.candidates().len() <= upper);
        }
        assert_eq!(candidate_count, stats.actual);
        assert!(stats.actual <= stats.theoretical);
    }
}

/// 枚举确定性:两个语法,两次运行产出逐条一致(码与模式)。
#[test]
fn enumeration_is_deterministic() {
    let words = word_code_analysis_entries();
    for spec in [
        CandidateEnumerationSpec::LEGACY_V1_FROZEN,
        CandidateEnumerationSpec::MONOTONE_V2_THEORETICAL,
    ] {
        let (targets_a, stats_a) = enumerate_targets_with_spec(&words, spec);
        let (targets_b, stats_b) = enumerate_targets_with_spec(&words, spec);
        assert_eq!(stats_a.actual, stats_b.actual);
        assert_eq!(stats_a.theoretical, stats_b.theoretical);
        assert_eq!(targets_a.len(), targets_b.len());
        for (a, b) in targets_a.iter().zip(targets_b.iter()) {
            assert_eq!(a.word(), b.word());
            assert_eq!(a.full_code(), b.full_code());
            assert_eq!(a.candidates().len(), b.candidates().len());
            for (ca, cb) in a.candidates().iter().zip(b.candidates().iter()) {
                assert_eq!(ca.shortcut_code(), cb.shortcut_code());
                assert_eq!(ca.mode().pattern(), cb.mode().pattern());
            }
        }
    }
}
