//! production ZERO_REGRESSION 选择 policy 的测试。
//!
//! 全部测试共享一份 evidence fixture(每个测试进程最多跑一次
//! ZERO_REGRESSION 30 次 normalized 主网格,避免 debug CI 时间放大)。

use std::collections::BTreeSet;
use std::sync::OnceLock;

use xhup_analyzer::frequency::{CharCodeUsage, FrequencyScale};
use xhup_analyzer::production::{ExclusionReason, ProductionEvidence, ProductionSelection};
use xhup_analyzer::sweep::OperatingPointId;
use xhup_analyzer::{AnalysisData, build_analysis, production};

/// 共享 fixture:分析输入 + 选择证据 + 选择结果。
struct Fixture {
    data: AnalysisData,
    evidence: ProductionEvidence,
    selection: ProductionSelection,
}

fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let data = build_analysis();
        let evidence = production::collect_evidence(&data);
        let selection = production::select_production_shortcuts(&evidence, &data.occupancy);
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

#[test]
fn reference_policy_is_typed_and_frozen() {
    // typed reference:balanced operating point + normalized 50:50 conservative。
    // identity 以 OperatingPointId 为准,不依赖数组位置或展示字符串。
    let balanced = OperatingPointId::Balanced.operating_point();
    assert_eq!(balanced.id, OperatingPointId::Balanced);
    assert_eq!(balanced.id.label(), "balanced");
    let reference = production::reference_cost();
    let expected = balanced.cost_model();
    assert_eq!(reference.selection_rank2_9, expected.selection_rank2_9);
    assert_eq!(
        reference.selection_rank10_plus,
        expected.selection_rank10_plus
    );
    assert_eq!(reference.ambiguity_coeff, expected.ambiguity_coeff);
    assert_eq!(reference.disruption_coeff, expected.disruption_coeff);
    assert_eq!(
        reference.mode_complexity_per_transition,
        expected.mode_complexity_per_transition
    );
    // evidence 的 reference 运行必须按 typed identity 定位。
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
        production::PRODUCTION_SHORTCUT_POLICY_VERSION,
        "zero-regression-high-v1"
    );
    // 整数阈值 4/5 对应 30 次运行 ≥ 24 票。
    assert_eq!(
        production::ROBUSTNESS_NUMERATOR * 30,
        24 * production::ROBUSTNESS_DENOMINATOR
    );
    assert_eq!(production::SENSITIVITY_RUNS, 30);
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
    // reference 本身是 ZERO_REGRESSION,baseline 占用排除恒为 0(否则 STOP)。
    assert_eq!(audit.excluded_by(ExclusionReason::BaselineOccupied), 0);
    assert!(audit.selected > 0);
}

#[test]
fn selection_satisfies_hard_invariants() {
    let fixture = fixture();
    let mut words = BTreeSet::new();
    let mut codes = BTreeSet::new();
    let mut word_codes = BTreeSet::new();
    for entry in &fixture.selection.selected {
        let full = entry.full_code.to_string();
        let shortcut = entry.shortcut_code.to_string();
        // baseline exact fanout == 0(ZERO_REGRESSION 本质,production 再断言)。
        assert_eq!(
            fixture.data.occupancy.fanout(&entry.shortcut_code),
            0,
            "{} {shortcut} 在 baseline 中必须空闲",
            entry.word
        );
        assert!(shortcut.len() >= 3 && shortcut.len() < full.len());
        assert!(shortcut.chars().all(|c| c.is_ascii_lowercase()));
        assert_eq!(entry.mode.chars().count(), entry.word.chars().count());
        // 机械投影一致性:shortcut == projection(full_code, mode)。
        assert_eq!(mechanical_projection(&full, &entry.mode), shortcut);
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
fn time_word_is_never_produced() {
    // 「时间」的 uij/ujm 在 baseline 中分别有 fanout 2/4,ZERO_REGRESSION 必须
    // 排除;它是未来 FIXED_FIRST 阶段的 sentinel,本层绝不出现。
    let fixture = fixture();
    assert!(
        !fixture
            .evidence
            .reference_assignments
            .iter()
            .any(|a| a.word == "时间"),
        "ZERO_REGRESSION reference 不应给「时间」分配 shortcut"
    );
    assert!(
        !fixture.selection.selected.iter().any(|e| e.word == "时间"),
        "production set 不应包含「时间」"
    );
}

#[test]
fn serialization_is_deterministic_and_canonical() {
    let fixture = fixture();
    let first = production::serialize_canonical_tsv(&fixture.selection.selected);
    let second = production::serialize_canonical_tsv(&fixture.selection.selected);
    assert_eq!(first, second, "序列化必须字节级确定");
    assert!(!first.contains('\r'), "canonical TSV 仅允许 LF");
    assert!(first.ends_with('\n') && !first.ends_with("\n\n"));
    assert!(first.contains(production::PRODUCTION_SHORTCUT_POLICY_VERSION));
    let rows: Vec<&str> = first
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect();
    assert_eq!(rows.len(), fixture.selection.selected.len());
    let mut previous: Option<(usize, &str, &str)> = None;
    for row in rows {
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 4, "每行 4 个 TAB 字段: {row}");
        let key = (fields[2].len(), fields[2], fields[0]);
        if let Some(prev) = previous {
            assert!(key > prev, "canonical 顺序: {row}");
        }
        previous = Some(key);
    }
}

#[test]
fn canonical_tsv_byte_reproduction() {
    // 入库 canonical TSV 必须能由 production selection API 字节级复现;
    // 不依赖 analyzer 之外的任何手工拼接。
    let fixture = fixture();
    let canonical = include_str!("../../../data/shortcuts/word_zero_regression.tsv");
    assert_eq!(
        production::serialize_canonical_tsv(&fixture.selection.selected),
        canonical,
        "canonical TSV 与 selection API 输出必须字节一致"
    );
}
