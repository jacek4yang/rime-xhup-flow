use xhup_analyzer::rules::*;
use xhup_core::knowledge::canonical_sources;
use xhup_core::rules::*;

#[test]
fn global_and_targeted_explanations_preserve_evidence_boundaries() {
    let sources = canonical_sources();
    let mut fixtures = parse_fixtures(CANONICAL_RULE_FIXTURES, sources).unwrap();
    let global = audit_rules(&fixtures, sources, None).unwrap();
    assert_eq!(global.global_counts["fixture_cases"], 49);
    assert_eq!(global.global_counts["regressions"], 0);
    assert_eq!(global.global_counts["intentional_deviations"], 2);
    assert_eq!(global.global_counts["unknown_unresolved_cases"], 2);
    fixtures.reverse();
    assert_eq!(
        serde_json::to_string(&global).unwrap(),
        serde_json::to_string(&audit_rules(&fixtures, sources, None).unwrap()).unwrap()
    );
    let character = audit_rules(&fixtures, sources, Some("嗯")).unwrap();
    assert_eq!(character.global_counts, global.global_counts);
    assert_eq!(character.official_expected_codes, ["enkx", "ogkx", "onkx"]);
    assert!(character.attested_codes.contains(&"ngkx".to_string()));
    assert!(
        character
            .knowledge
            .unwrap()
            .normalized_evidence
            .contains("reading\t嗯\tn\tprimary")
    );
    let word = audit_rules(&fixtures, sources, Some("提示词")).unwrap();
    assert!(word.official_expected_codes.is_empty());
    assert_eq!(word.flow_extension_codes, ["tiuici"]);
    assert!(
        word.explanations
            .iter()
            .any(|e| e.expected_code.as_deref() == Some("tuci")
                && e.compatibility_class == "rule-compatible")
    );
    let unknown = audit_rules(&fixtures, sources, Some("未知测试目标")).unwrap();
    assert_eq!(unknown.counts["unknown_target"], 1);
    assert!(unknown.official_expected_codes.is_empty());
}

#[test]
fn targeted_report_cannot_hide_an_unrelated_protected_regression() {
    let data = CANONICAL_RULE_FIXTURES.replace("\txn,ld\txnld\t", "\tzz,zz\tzzzz\t");
    let fixtures = parse_fixtures(&data, canonical_sources()).unwrap();
    let report = audit_rules(&fixtures, canonical_sources(), Some("提示词")).unwrap();
    assert_eq!(report.counts["regressions"], 0);
    assert_eq!(report.global_counts["regressions"], 1);
    assert_eq!(
        report.global_counts["missing_expected_official_relations"],
        3
    );
}

#[test]
fn formal_policy_projection_matches_the_existing_v2_compiler() {
    use xhup_analyzer::candidates::{CandidateEnumerationSpec, enumerate_targets_with_spec};
    let fixtures = parse_fixtures(CANONICAL_RULE_FIXTURES, canonical_sources()).unwrap();
    let policy: Vec<_> = fixtures
        .iter()
        .filter(|f| matches!(f.rule, RuleId::MonotoneShortcut | RuleId::LegacyShortcut))
        .collect();
    let words: Vec<_> = xhup_generator::word_code_analysis_entries()
        .into_iter()
        .filter(|w| policy.iter().any(|f| f.text == w.word()))
        .collect();
    for (rule, spec) in [
        (
            RuleId::MonotoneShortcut,
            CandidateEnumerationSpec::MONOTONE_V2_THEORETICAL,
        ),
        (
            RuleId::LegacyShortcut,
            CandidateEnumerationSpec::LEGACY_V1_FROZEN,
        ),
    ] {
        let (targets, _) = enumerate_targets_with_spec(&words, spec);
        for fixture in policy.iter().filter(|f| f.rule == rule) {
            assert!(
                targets
                    .iter()
                    .filter(|t| t.word() == fixture.text)
                    .any(|t| t
                        .candidates()
                        .iter()
                        .any(|c| Some(c.shortcut_code()) == fixture.expected.as_ref())),
                "{}",
                fixture.id
            );
        }
    }
}

#[test]
fn cli_json_is_stable_and_arguments_are_strict() {
    let run = || {
        std::process::Command::new(env!("CARGO_BIN_EXE_xhup-rule-audit"))
            .args(["--word", "提示词", "--json", "--check"])
            .output()
            .unwrap()
    };
    let first = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(first.stdout, run().stdout);
    let report: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(report["flow_extension_codes"][0], "tiuici");
    for args in [
        vec!["--char", "甲乙"],
        vec!["--word"],
        vec!["--unknown"],
        vec!["--char", "甲", "--word", "乙"],
        vec!["--word", " "],
    ] {
        assert!(
            !std::process::Command::new(env!("CARGO_BIN_EXE_xhup-rule-audit"))
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
}
