//! production FIXED_FIRST 选择 policy 的测试(含 incremental universe 的
//! synthetic 机制测试)。
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
use xhup_analyzer::{
    AnalysisData, CandidateEnumerationSpec, CandidateGrammar, ShortcutCandidate, WordTarget,
    build_analysis_with_spec, production,
};
use xhup_core::KeySequence;
use xhup_generator::{canonical_word_code_entries, canonical_word_shortcut_entries};

/// 共享 fixture:分析输入 + 增量证据 + 选择结果。
///
/// PR #23 FIXED_FIRST 证据绑定 Monotone V2 理论全集枚举规格(production
/// min-length 由 policy 在 universe 构造时过滤)。
struct Fixture {
    data: AnalysisData,
    evidence: FixedFirstEvidence,
    selection: FixedFirstProductionSelection,
}

fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let data = build_analysis_with_spec(CandidateEnumerationSpec::MONOTONE_V2_THEORETICAL);
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
    // production evidence 显式暴露候选语法身份(不允许隐式)。
    assert_eq!(
        fixture().evidence.candidate_grammar,
        CandidateGrammar::MonotoneSuffixInitialsV2,
        "FIXED_FIRST reference 必须绑定 MonotoneSuffixInitialsV2"
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
    // production 最短长度是 policy 层(1/2 键保留),不是语法层。
    assert_eq!(production_fixed_first::PRODUCTION_MIN_SHORTCUT_LENGTH, 3);
    // 整数阈值 4/5 对应 30 次运行 ≥ 24 票。
    assert_eq!(
        production::ROBUSTNESS_NUMERATOR * 30,
        24 * production::ROBUSTNESS_DENOMINATOR
    );
    assert_eq!(production::SENSITIVITY_RUNS, 30);
}

#[test]
fn universe_is_incremental_and_colliding_only() {
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
    // 候选在优化前已被限制为长度 >= 3 且 baseline fanout > 0 的重码候选
    //(无上限);production min-length 是 policy 过滤,不是语法语义。
    for target in &targets {
        for candidate in target.candidates() {
            let fanout = fixture.data.occupancy.fanout(candidate.shortcut_code());
            assert!(
                fanout > 0,
                "{} {} baseline fanout 必须 > 0",
                target.word(),
                candidate.shortcut_code(),
            );
            assert!(
                candidate.shortcut_code().len()
                    >= production_fixed_first::PRODUCTION_MIN_SHORTCUT_LENGTH,
                "{} {} 长度低于 production 最短长度",
                target.word(),
                candidate.shortcut_code()
            );
        }
    }
    assert_eq!(
        stats.original_targets,
        stats.zr_words_excluded + stats.remaining_targets
    );
    assert!(
        stats.below_min_length_candidates > 0,
        "2-key 理论候选(如 时间 uj/II)应被 policy 过滤计数"
    );
}

/// 语法合法性 vs production 资格的架构回归:「时间 → uj/II」在 Monotone
/// V2 语法层结构性合法(存在于理论候选),但被 production min-length
/// policy 排除,不得进入 candidate universe 与 canonical TSV。
#[test]
fn two_key_candidate_is_legal_grammar_but_not_production_eligible() {
    let fixture = fixture();
    // 语法层:理论候选恰为 uij/FI 与 uj/II。
    let time_target = fixture
        .data
        .targets
        .iter()
        .find(|t| t.word() == "时间")
        .expect("时间必然在词表中");
    let candidates: Vec<(String, String)> = time_target
        .candidates()
        .iter()
        .map(|c| (c.shortcut_code().to_string(), c.mode().pattern()))
        .collect();
    assert_eq!(
        candidates,
        vec![
            ("uij".to_string(), "FI".to_string()),
            ("uj".to_string(), "II".to_string())
        ]
    );
    // production universe 层:uj 被排除,ujm 结构性不存在。
    let (targets, _) = production_fixed_first::build_fixed_first_universe(&fixture.data);
    let time_universe = targets
        .iter()
        .find(|t| t.word() == "时间")
        .expect("时间不在 ZR 词表中,必然保留");
    let codes: Vec<String> = time_universe
        .candidates()
        .iter()
        .map(|c| c.shortcut_code().to_string())
        .collect();
    assert!(
        !codes.contains(&"uj".to_string()),
        "uj 不得进入 production universe"
    );
    assert!(
        !codes.contains(&"ujm".to_string()),
        "ujm 在 Monotone V2 下结构性非法"
    );
    // canonical TSV 层:uj 不得入库。
    assert!(
        !fixture
            .selection
            .selected
            .iter()
            .any(|e| e.word == "时间" && e.shortcut_code.len() < 3),
        "production 选择不得含长度 < 3 的码"
    );
}

/// production 选择集模式审计:全部选中模式必须属于 Monotone V2(2 字 FI;
/// 3 字 FFI/FII/III;4 字 FFFI/FFII/FIII/IIII)。任何含 I…F 的模式是硬失败。
#[test]
fn production_patterns_are_exhaustively_monotone() {
    let fixture = fixture();
    let legal: [&str; 9] = [
        "FI", "II", "FFI", "FII", "III", "FFFI", "FFII", "FIII", "IIII",
    ];
    for entry in &fixture.selection.selected {
        assert!(
            legal.contains(&entry.mode.as_str()),
            "{} 的模式 {} 不属于 Monotone V2 合法集",
            entry.word,
            entry.mode
        );
        assert!(
            entry.mode.chars().all(|c| matches!(c, 'F' | 'I')),
            "{} 的模式 {} 含非法字符",
            entry.word,
            entry.mode
        );
        // 结构性重验:一旦 I 出现,后续不得再 F。
        let mut seen_initial = false;
        for c in entry.mode.chars() {
            match c {
                'I' => seen_initial = true,
                'F' => assert!(
                    !seen_initial,
                    "{} 的模式 {} 含 I…F(非单调)",
                    entry.word, entry.mode
                ),
                _ => unreachable!(),
            }
        }
    }
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
        // baseline fanout > 0(无上限),期望名次 = fanout + 1。
        let fanout = fixture.data.occupancy.fanout(&entry.shortcut_code);
        assert_eq!(fanout, entry.baseline_fanout);
        assert!(fanout > 0);
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
    // ujm 行不存在(一词最多一码);且 ujm/IF 在 Monotone V2 candidate
    // universe 中结构性不存在(不是"票数不够",是非法模式)。
    assert!(
        !fixture
            .selection
            .selected
            .iter()
            .any(|e| e.word == "时间" && e.shortcut_code == key("ujm")),
        "「时间」不得同时持有 ujm 简码"
    );
    assert_ne!(entry.mode, "IF", "IF 不是 Monotone V2 合法模式");
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
    assert!(
        first.contains("# candidate grammar: monotone-suffix-initials-v2"),
        "canonical TSV 头必须显式记录候选语法"
    );
    assert!(
        first.contains("# production min shortcut length: 3"),
        "canonical TSV 头必须显式记录 production 最短长度"
    );
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
        enumeration_spec: CandidateEnumerationSpec::MONOTONE_V2_THEORETICAL,
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
fn deep_fanout_candidates_are_kept() {
    // policy: candidate universe 为 baseline fanout > 0,不设上限。
    // 深码候选(fanout > 8)必须保留在 universe 中参与优化,
    // 由 30 次增量运行 + 4/5 整数门限决定是否入选,不做 pre-filter。
    // 注:深码示例必须满足 production min-length(>= 3);2-key 深码
    //(如 yi)由 min-length policy 排除,那是另一条测试的职责。
    let fixture = fixture();
    let deep = key("uia");
    let deep_fanout = fixture.data.occupancy.fanout(&deep);
    assert!(deep_fanout > 8, "测试前提:uia baseline fanout 必须 > 8");
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
                ShortcutCandidate::for_test(key("uij")),
            ],
        )],
        enumeration: Default::default(),
        enumeration_spec: CandidateEnumerationSpec::MONOTONE_V2_THEORETICAL,
        frequency: fixture.data.frequency.clone(),
    };
    let (targets, _) = production_fixed_first::build_fixed_first_universe(&data);
    let candidates = targets[0].candidates();
    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.shortcut_code() == &deep),
        "深码候选(fanout={deep_fanout})必须保留在 universe 中"
    );
    assert_eq!(candidates.len(), 2, "两个重码候选都应保留");
}
