//! 优化器测试:唯一性、确定性、profile 门禁、硬保护、收益不变量。

use std::collections::BTreeSet;
use std::sync::OnceLock;

use xhup_analyzer::occupancy::CandidateSource;
use xhup_analyzer::optimize::{OptimizationProfile, optimize};
use xhup_analyzer::report::{balanced_cost, balanced_scale};
use xhup_analyzer::{AnalysisData, build_analysis};

fn data() -> &'static AnalysisData {
    static DATA: OnceLock<AnalysisData> = OnceLock::new();
    DATA.get_or_init(build_analysis)
}

#[test]
fn zero_regression_assignments_are_unique_and_on_empty_codes() {
    let data = data();
    let outcome = optimize(
        &data.targets,
        &data.occupancy,
        &data.frequency,
        &balanced_scale(),
        &balanced_cost(),
        OptimizationProfile::ZeroRegression,
    );
    assert!(!outcome.assignments.is_empty(), "应存在零回归推荐");
    let mut words = BTreeSet::new();
    let mut codes = BTreeSet::new();
    for assignment in &outcome.assignments {
        assert!(
            words.insert(&assignment.word),
            "词唯一: {}",
            assignment.word
        );
        assert!(
            codes.insert(assignment.evaluation.shortcut_code.clone()),
            "码唯一: {}",
            assignment.evaluation.shortcut_code
        );
        // ZERO_REGRESSION:exact code 空闲,rank 1,fanout 1
        assert_eq!(assignment.evaluation.existing_fanout, 0);
        assert_eq!(assignment.evaluation.projected_rank, 1);
        assert_eq!(assignment.evaluation.projected_fanout, 1);
        assert!(assignment.evaluation.breakdown.net_utility > 0.0);
        assert!(assignment.evaluation.shortcut_code.len() >= 3);
    }
    // 统计一致性
    assert_eq!(outcome.stats.assigned_words, outcome.assignments.len());
    assert_eq!(outcome.stats.exact_code_collisions, 0);
    assert!(outcome.disruptions.is_empty());
    assert!(outcome.stats.weighted_keys_saved() > 0.0);
}

#[test]
fn optimizer_output_is_deterministic() {
    let data = data();
    for profile in OptimizationProfile::all() {
        let a = optimize(
            &data.targets,
            &data.occupancy,
            &data.frequency,
            &balanced_scale(),
            &balanced_cost(),
            profile,
        );
        let b = optimize(
            &data.targets,
            &data.occupancy,
            &data.frequency,
            &balanced_scale(),
            &balanced_cost(),
            profile,
        );
        assert_eq!(a.assignments.len(), b.assignments.len());
        for (x, y) in a.assignments.iter().zip(b.assignments.iter()) {
            assert_eq!(x.word, y.word);
            assert_eq!(x.evaluation.shortcut_code, y.evaluation.shortcut_code);
            assert_eq!(
                x.evaluation.breakdown.net_utility.to_bits(),
                y.evaluation.breakdown.net_utility.to_bits()
            );
        }
    }
}

#[test]
fn fixed_first_appends_after_all_existing_candidates() {
    let data = data();
    let outcome = optimize(
        &data.targets,
        &data.occupancy,
        &data.frequency,
        &balanced_scale(),
        &balanced_cost(),
        OptimizationProfile::FixedFirst,
    );
    assert!(!outcome.assignments.is_empty());
    assert!(
        outcome.disruptions.is_empty(),
        "FIXED_FIRST 不产生既有候选扰动"
    );
    for assignment in &outcome.assignments {
        let e = &assignment.evaluation;
        assert_eq!(
            e.projected_rank as usize,
            e.existing_fanout + 1,
            "{} {} 应追加到组尾",
            assignment.word,
            e.shortcut_code
        );
        assert_eq!(e.breakdown.disruption_cost, 0.0);
    }
}

#[test]
fn empty_length_only_uses_only_five_or_seven_key_codes() {
    let data = data();
    let outcome = optimize(
        &data.targets,
        &data.occupancy,
        &data.frequency,
        &balanced_scale(),
        &balanced_cost(),
        OptimizationProfile::EmptyLengthOnly,
    );
    assert!(!outcome.assignments.is_empty(), "5/7 键空闲层应有推荐");
    for assignment in &outcome.assignments {
        let length = assignment.evaluation.shortcut_code.len();
        assert!(
            matches!(length, 5 | 7),
            "只允许 5/7 键: {}",
            assignment.evaluation.shortcut_code
        );
        assert_eq!(assignment.evaluation.existing_fanout, 0);
    }
}

#[test]
fn optimized_respects_hard_protections() {
    let data = data();
    let outcome = optimize(
        &data.targets,
        &data.occupancy,
        &data.frequency,
        &balanced_scale(),
        &balanced_cost(),
        OptimizationProfile::Optimized,
    );
    assert!(!outcome.assignments.is_empty());
    for assignment in &outcome.assignments {
        let code = &assignment.evaluation.shortcut_code;
        // 长度 ≥3 蕴含 1/2 键不可达;显式再断一次
        assert!(code.len() >= 3, "1/2 键空间不可达: {code}");
        if code.len() == 4
            && let Some(group) = data.occupancy.group(code)
        {
            let char_entries = group
                .iter()
                .filter(|c| c.source() == CandidateSource::CharCode)
                .count();
            if char_entries > 0 {
                // 4 键规范全码硬保护:shortcut 只能追加到组尾
                assert_eq!(
                    assignment.evaluation.projected_rank as usize,
                    assignment.evaluation.existing_fanout + 1,
                    "4 键全码组 {} 的 top 不可被 {} 挤掉",
                    code,
                    assignment.word
                );
            }
        }
    }
}
