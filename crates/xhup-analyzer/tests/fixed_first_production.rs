//! production FIXED_FIRST 选择 policy 的测试(含 incremental universe 与
//! fanout cap 的 synthetic 机制测试)。
//!
//! 真实数据测试共享一份 evidence fixture(每个测试进程最多跑一次
//! FIXED_FIRST 30 次 normalized 增量主网格,避免 debug CI 时间放大)。

use std::collections::BTreeSet;
use std::sync::OnceLock;

use xhup_analyzer::frequency::{CharCodeUsage, FrequencyScale};
use xhup_analyzer::occupancy::CodeOccupancy;
use xhup_analyzer::optimize::{OptimizationProfile, optimize};
use xhup_analyzer::production_fixed_first::{
    self, FixedFirstEvidence, FixedFirstExclusionReason, FixedFirstProductionSelection,
};
use xhup_analyzer::sweep::OperatingPointId;
use xhup_analyzer::{AnalysisData, ShortcutCandidate, WordTarget, build_analysis, production};
use xhup_core::KeySequence;
use xhup_generator::{canonical_word_code_entries, canonical_word_shortcut_entries};

/// 共享 fixture:分析输入 + 增量证据 + 选择结果。
struct Fixture {
    data: AnalysisData,
    evidence: FixedFirstEvidence,
    selection: FixedFirstProductionSelection,
}

fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let data = build_analysis();
        let evidence = production_fixed_first::collect_fixed_first_evidence(&data);
        // 兼容性参考系:baseline + ZERO_REGRESSION 层(不含 FF 层自身)。
        let pre_fixed_first_current = CodeOccupancy::build_pre_fixed_first_production();
        let selection = production_fixed_first::select_fixed_first_production(
            &evidence,
            &data.occupancy,
            &pre_fixed_first_current,
        );
        Fixture {
            data,
            evidence,
            selection,
        }
    })
}

/// 由完整码与 F/I 模式机械重算 shortcut(与候选枚举同一 frozen rule)。
fn mechanical_projection(full_code: &str, mode: &str) -> String {
    let keys: Vec<char> = full_code.chars().collect();
    let (chunks, _) = keys.as_chunks::<2>();
    assert_eq!(chunks.len(), mode.chars().count(), "模式数应等于字数");
    let mut out = String::new();
    for (chunk, mode_char) in chunks.iter().zip(mode.chars()) {
        match mode_char {
            'F' => out.extend([chunk[0], chunk[1]]),
            'I' => out.push(chunk[0]),
            other => panic!("模式只含 F/I: {other}"),
        }
    }
    out
}

fn key(code: &str) -> KeySequence {
    code.parse().expect("测试码合法")
}

#[test]
fn reference_policy_is_typed_and_frozen() {
    // typed reference:OperatingPointId::Balanced + normalized 50:50
    // conservative,不依赖数组位置或展示字符串。
    assert_eq!(
        fixture().evidence.reference_point,
        OperatingPointId::Balanced
    );
    assert!(matches!(
        production::reference_scale(),
        FrequencyScale::Normalized { char_share, usage }
            if char_share == 0.5 && usage == CharCodeUsage::Conservative
    ));
    assert_eq!(
        production_fixed_first::FIXED_FIRST_PRODUCTION_POLICY_VERSION,
        "fixed-first-high-v1"
    );
    assert_eq!(production_fixed_first::FIXED_FIRST_MAX_BASELINE_FANOUT, 8);
    // 整数阈值 4/5 对应 30 次运行 ≥ 24 票。
    assert_eq!(
        production::ROBUSTNESS_NUMERATOR * 30,
        24 * production::ROBUSTNESS_DENOMINATOR
    );
    assert_eq!(production::SENSITIVITY_RUNS, 30);
}

#[test]
fn universe_is_incremental_and_fanout_capped() {
    let fixture = fixture();
    let (targets, stats) = production_fixed_first::build_fixed_first_universe(&fixture.data);
    let zr_words: BTreeSet<&str> = canonical_word_shortcut_entries()
        .iter()
        .map(|entry| entry.word())
        .collect();
    // 全部 ZR production 词在优化前被移除。
    assert_eq!(stats.zr_words_excluded, zr_words.len());
    assert!(
        targets
            .iter()
            .all(|target| !zr_words.contains(target.word())),
        "incremental universe 不得包含任何 ZR production 词"
    );
    // 候选在优化前已被限制为 1..=8 baseline fanout 的重码候选。
    for target in &targets {
        for candidate in target.candidates() {
            let fanout = fixture.data.occupancy.fanout(candidate.shortcut_code());
            assert!(
                (1..=production_fixed_first::FIXED_FIRST_MAX_BASELINE_FANOUT).contains(&fanout),
                "{} {} baseline fanout {fanout} 超出 1..=8",
                target.word(),
                candidate.shortcut_code(),
            );
        }
    }
    assert_eq!(
        stats.original_targets,
        stats.zr_words_excluded + stats.remaining_targets
    );
}

#[test]
fn audit_arithmetic_is_complete() {
    let fixture = fixture();
    let audit = &fixture.selection.audit;
    assert_eq!(
        audit.reference_assignments,
        audit.selected + audit.excluded_total(),
        "每条 reference assignment 必须被选中或按原因排除"
    );
    // 结构性排除原因恒为 0(incremental universe + ZR 冻结已保证)。
    use FixedFirstExclusionReason as Reason;
    for reason in [
        Reason::AlreadyZeroRegressionWord,
        Reason::NoCollidingCandidate,
        Reason::FanoutAboveProductionCap,
        Reason::CurrentOccupancyMismatch,
        Reason::ZeroRegressionCodeConflict,
    ] {
        assert_eq!(audit.excluded_by(reason), 0, "{reason:?} 必须为 0");
    }
    assert!(!fixture.selection.selected.is_empty());
}

#[test]
fn selection_satisfies_hard_invariants() {
    let fixture = fixture();
    let zr_words: BTreeSet<&str> = canonical_word_shortcut_entries()
        .iter()
        .map(|entry| entry.word())
        .collect();
    let zr_codes: BTreeSet<String> = canonical_word_shortcut_entries()
        .iter()
        .map(|entry| entry.shortcut_code().to_string())
        .collect();
    let canonical_entries = canonical_word_code_entries();
    let canonical_pairs: BTreeSet<(&str, String)> = canonical_entries
        .iter()
        .map(|entry| (entry.word(), entry.code().to_string()))
        .collect();
    let mut words = BTreeSet::new();
    let mut codes = BTreeSet::new();
    let mut word_codes = BTreeSet::new();
    for entry in &fixture.selection.selected {
        let full = entry.full_code.to_string();
        let shortcut = entry.shortcut_code.to_string();
        // 与 ZR production 词/码集合全量不相交。
        assert!(
            !zr_words.contains(entry.word.as_str()),
            "{} 已有 ZR 简码",
            entry.word
        );
        assert!(
            !zr_codes.contains(&shortcut),
            "{} {shortcut} 与 ZR 码冲突",
            entry.word
        );
        // baseline fanout 1..=8,期望名次 = fanout + 1。
        let fanout = fixture.data.occupancy.fanout(&entry.shortcut_code);
        assert_eq!(fanout, entry.baseline_fanout);
        assert!((1..=8).contains(&fanout));
        assert_eq!(entry.expected_rank, fanout + 1);
        // 形式不变量。
        assert!(shortcut.len() >= 3 && shortcut.len() < full.len());
        assert!(shortcut.chars().all(|c| c.is_ascii_lowercase()));
        assert_eq!(entry.mode.chars().count(), entry.word.chars().count());
        assert_eq!(mechanical_projection(&full, &entry.mode), shortcut);
        // 完整码仍是 canonical 编码关系(完整码别名保留)。
        assert!(
            canonical_pairs.contains(&(entry.word.as_str(), full.clone())),
            "{} {full} 不在 canonical 词编码层",
            entry.word
        );
        assert!(words.insert(&entry.word), "词重复: {}", entry.word);
        assert!(codes.insert(shortcut.clone()), "码重复: {shortcut}");
        assert!(word_codes.insert((entry.word.clone(), full.clone())));
        assert_eq!(entry.total_runs, production::SENSITIVITY_RUNS);
    }
}

#[test]
fn robustness_gate_integrity() {
    let fixture = fixture();
    for entry in &fixture.selection.selected {
        let record = fixture
            .evidence
            .robustness
            .get(&entry.word)
            .expect("被选词必须有 robustness 记录");
        let (top_code, votes) = record.top_code().expect("被选词必须有 top code");
        assert_eq!(top_code, entry.shortcut_code.to_string());
        assert_eq!(votes, entry.top_code_votes);
        // 整数交叉乘法阈值。
        assert!(
            votes * production::ROBUSTNESS_DENOMINATOR
                >= record.total_runs * production::ROBUSTNESS_NUMERATOR,
            "{}: {}/{} 低于 4/5",
            entry.word,
            votes,
            record.total_runs
        );
    }
}

#[test]
fn pre_fixed_first_current_matches_baseline_fanout() {
    // 兼容性断言:PR #22 ZR 码理论上全部 baseline fanout == 0,故每个
    // FIXED_FIRST 码在 pre-FIXED_FIRST production(baseline + ZR 层)中的
    // fanout 与 baseline 完全一致;否则说明 ZR 码与本层码发生不应有的
    // exact overlap。
    let fixture = fixture();
    let pre_fixed_first_current = CodeOccupancy::build_pre_fixed_first_production();
    for entry in &fixture.selection.selected {
        assert_eq!(
            pre_fixed_first_current.fanout(&entry.shortcut_code),
            entry.baseline_fanout,
            "{} {} 在 pre-FIXED_FIRST production 中 fanout 应与 baseline 相同",
            entry.word,
            entry.shortcut_code,
        );
    }
}

#[test]
fn time_word_sentinel() {
    // 「时间」(full uijm)是增量 FIXED_FIRST policy 的核心回归锚点:
    // uij(baseline 铈/鼫,fanout 2)应以 >= 4/5 同码票数被选中;
    // ujm 不得同时入库(一词最多一码)。
    let fixture = fixture();
    let selected: Vec<_> = fixture
        .selection
        .selected
        .iter()
        .filter(|e| e.word == "时间")
        .collect();
    assert_eq!(selected.len(), 1, "「时间」应恰有一条 production 简码");
    let entry = selected[0];
    assert_eq!(entry.full_code, key("uijm"));
    assert_eq!(entry.shortcut_code, key("uij"));
    assert_eq!(entry.mode, "FI");
    assert_eq!(entry.baseline_fanout, 2);
    assert_eq!(entry.expected_rank, 3);
    assert!(
        entry.top_code_votes * production::ROBUSTNESS_DENOMINATOR
            >= entry.total_runs * production::ROBUSTNESS_NUMERATOR
    );
    // ujm 行不存在。
    assert!(
        !fixture
            .selection
            .selected
            .iter()
            .any(|e| e.word == "时间" && e.shortcut_code == key("ujm")),
        "「时间」不得同时持有 ujm 简码"
    );
}

#[test]
fn serialization_is_deterministic_and_canonical() {
    let fixture = fixture();
    let first = production_fixed_first::serialize_fixed_first_tsv(&fixture.selection.selected);
    let second = production_fixed_first::serialize_fixed_first_tsv(&fixture.selection.selected);
    assert_eq!(first, second, "序列化必须字节级确定");
    assert!(!first.contains('\r'), "canonical TSV 仅允许 LF");
    assert!(first.ends_with('\n') && !first.ends_with("\n\n"));
    assert!(first.contains(production_fixed_first::FIXED_FIRST_PRODUCTION_POLICY_VERSION));
    let rows: Vec<&str> = first
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect();
    assert_eq!(rows.len(), fixture.selection.selected.len());
    let mut previous: Option<(usize, &str, &str, &str, &str)> = None;
    for row in rows {
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 4, "每行 4 个 TAB 字段: {row}");
        let key = (fields[2].len(), fields[2], fields[0], fields[1], fields[3]);
        if let Some(prev) = previous {
            assert!(key > prev, "canonical 顺序: {row}");
        }
        previous = Some(key);
    }
}

#[test]
fn canonical_tsv_byte_reproduction() {
    // 入库 canonical TSV 必须能由 production selection API 字节级复现。
    let fixture = fixture();
    let canonical = include_str!("../../../data/shortcuts/word_fixed_first.tsv");
    assert_eq!(
        production_fixed_first::serialize_fixed_first_tsv(&fixture.selection.selected),
        canonical,
        "canonical TSV 与 selection API 输出必须字节一致"
    );
}

#[test]
fn zr_word_is_removed_before_optimization() {
    // synthetic P0:已有 ZR 简码的词必须在优化前移除,而不是先占码再过滤。
    // A = 真实 ZR 词「就是」携带 uij 候选;B = 非 ZR 词携带同一候选。
    // universe 必须只含 B,且 uij 分配给 B(若 A 先占码后过滤,B 将丢失该码)。
    let fixture = fixture();
    let a = "就是";
    assert!(
        canonical_word_shortcut_entries()
            .iter()
            .any(|entry| entry.word() == a),
        "测试前提:{a} 必须是 ZR production 词"
    );
    let b = "时间";
    assert!(
        !canonical_word_shortcut_entries()
            .iter()
            .any(|entry| entry.word() == b),
        "测试前提:{b} 不得是 ZR production 词"
    );
    // optimizer 要求词真实占用其完整码组;A 在优化前被移除,不参与 optimize。
    let data = AnalysisData {
        chars: Vec::new(),
        words: Vec::new(),
        occupancy: fixture.data.occupancy.clone(),
        targets: vec![
            WordTarget::with_candidates_for_test(
                a,
                key("jqui"),
                1_000_000,
                vec![ShortcutCandidate::for_test(key("uij"))],
            ),
            WordTarget::with_candidates_for_test(
                b,
                key("uijm"),
                500_000,
                vec![ShortcutCandidate::for_test(key("uij"))],
            ),
        ],
        enumeration: Default::default(),
        frequency: fixture.data.frequency.clone(),
    };
    let (targets, _) = production_fixed_first::build_fixed_first_universe(&data);
    assert_eq!(targets.len(), 1, "ZR 词必须在优化前被移除");
    assert_eq!(targets[0].word(), b);
    let outcome = optimize(
        &targets,
        &data.occupancy,
        &data.frequency,
        &production::reference_scale(),
        &production::reference_cost(),
        OptimizationProfile::FixedFirst,
    );
    let assignment = outcome
        .assignments
        .iter()
        .find(|assignment| assignment.word == b)
        .expect("B 应获得 uij 分配");
    assert_eq!(assignment.evaluation.shortcut_code, key("uij"));
}

#[test]
fn fanout_cap_is_applied_before_optimization() {
    // synthetic P0:fanout > 8 的深码候选必须在优化前移除,不得先参与
    // greedy allocation 再在 production gate 后删除。
    let fixture = fixture();
    let deep = key("yi");
    let shallow = key("uij");
    let deep_fanout = fixture.data.occupancy.fanout(&deep);
    assert!(
        deep_fanout > production_fixed_first::FIXED_FIRST_MAX_BASELINE_FANOUT,
        "测试前提:yi baseline fanout {deep_fanout} 必须超过 cap"
    );
    assert_eq!(fixture.data.occupancy.fanout(&shallow), 2);
    // 用真实词「时间」(uijm baseline 组 rank 1):optimizer 要求词真实
    // 占用其完整码组。
    let word = "时间";
    let data = AnalysisData {
        chars: Vec::new(),
        words: Vec::new(),
        occupancy: fixture.data.occupancy.clone(),
        targets: vec![WordTarget::with_candidates_for_test(
            word,
            key("uijm"),
            500_000,
            vec![
                ShortcutCandidate::for_test(deep.clone()),
                ShortcutCandidate::for_test(shallow.clone()),
            ],
        )],
        enumeration: Default::default(),
        frequency: fixture.data.frequency.clone(),
    };
    let (targets, stats) = production_fixed_first::build_fixed_first_universe(&data);
    assert_eq!(stats.candidates_rejected_fanout_above_cap, 1);
    let candidates = targets[0].candidates();
    assert_eq!(candidates.len(), 1, "深码候选必须在优化前被移除");
    assert_eq!(candidates[0].shortcut_code(), &shallow);
    let outcome = optimize(
        &targets,
        &data.occupancy,
        &data.frequency,
        &production::reference_scale(),
        &production::reference_cost(),
        OptimizationProfile::FixedFirst,
    );
    let assignment = outcome
        .assignments
        .iter()
        .find(|assignment| assignment.word == word)
        .expect("目标应获得浅码分配");
    assert_eq!(assignment.evaluation.shortcut_code, shallow);
}
