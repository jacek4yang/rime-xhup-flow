use std::collections::BTreeSet;
use xhup_analyzer::knowledge::*;
use xhup_core::{InputHanzi, XhupHanzi, knowledge::*};

#[test]
fn global_audit_and_character_explanation_match_canonical_evidence() {
    let e = xhup_generator::knowledge::canonical_knowledge().unwrap();
    let core = XhupHanzi::all().iter().map(|c| c.as_char()).collect();
    let input = InputHanzi::all().iter().map(|c| c.as_char()).collect();
    let report = audit_knowledge(&e, canonical_sources(), &core, &input, 100_000);
    for (name, expected) in [
        ("input_characters", 8208),
        ("core_characters", 8105),
        ("reading_evidence", 8580),
        ("full_evidence", 9796),
        ("resolved_full_relations", 9873),
        ("outside_core", 103),
        ("duplicate_evidence", 0),
        ("invalid_provenance", 0),
        ("unsupported_trusted", 0),
    ] {
        assert_eq!(report.counts[name], expected, "{name}");
    }
    let explanation = explain_character('嗯', &e, canonical_sources(), &core, &input).unwrap();
    assert!(explanation.core_standard && explanation.input_member);
    assert_eq!(
        explanation.resolved_full_codes,
        ["enkx", "ngkx", "ogkx", "onkx"]
    );
    assert_eq!(explanation.preferred_sources["enkx"], "flypy-official-ix");
    assert!(
        explanation
            .normalized_evidence
            .contains("reading\t嗯\tn\tprimary")
    );
    assert!(
        !explanation
            .normalized_evidence
            .contains("reading\t嗯\ten\t")
    );
    let serialized = serialize_evidence(&e).unwrap();
    assert_eq!(
        serialize_evidence(&parse_evidence(&serialized).unwrap()).unwrap(),
        serialized
    );
    let mut reversed = e.clone();
    reversed.full_codes.reverse();
    reversed.sounds.reverse();
    assert_eq!(serialize_evidence(&reversed).unwrap(), serialized);
    assert_eq!(
        audit_knowledge(&reversed, canonical_sources(), &core, &input, 100_000),
        report
    );
    let production_full: BTreeSet<_> = xhup_generator::canonical_input_char_code_entries()
        .into_iter()
        .filter(|r| r.code().len() == 4)
        .map(|r| (r.hanzi().as_char(), r.code().to_string()))
        .collect();
    let resolved: BTreeSet<_> = resolve_full_codes(&e.full_codes, canonical_sources())
        .unwrap()
        .into_iter()
        .map(|r| (r.character.0, r.code.to_string()))
        .chain(
            e.generated
                .iter()
                .map(|r| (r.character.0, r.code.to_string())),
        )
        .collect();
    assert_eq!(resolved, production_full);
}

#[test]
fn synthetic_audit_reports_gaps_conflicts_duplicates_and_orphans() {
    let mut e = KnowledgeEvidence::default();
    let p = Provenance {
        source: "rime-fast-xhup",
        status: EvidenceStatus::Attested,
        confidence: EvidenceConfidence::High,
    };
    e.sounds.push(SoundCodeEvidence {
        character: CharacterId('甲'),
        code: "aa".parse().unwrap(),
        provenance: p,
    });
    e.sounds.push(SoundCodeEvidence {
        character: CharacterId('甲'),
        code: "bb".parse().unwrap(),
        provenance: p,
    });
    e.sounds.push(e.sounds[0]);
    e.shapes.push(ShapeCodeEvidence {
        character: CharacterId('乙'),
        code: "aa".parse().unwrap(),
        provenance: Provenance {
            source: "orphan",
            ..p
        },
    });
    e.frequencies.push(FrequencyEvidence {
        character: CharacterId('甲'),
        reading: Some("jia"),
        value: 100,
        kind: FrequencyKind::ReadingScore,
        provenance: p,
    });
    let report = audit_knowledge(
        &e,
        canonical_sources(),
        &BTreeSet::new(),
        &BTreeSet::new(),
        100,
    );
    for category in [
        "unsupported_trusted",
        "sound_without_shape",
        "shape_without_sound",
        "conflicting_sound",
        "duplicate_evidence",
        "invalid_provenance",
        "high_frequency_without_input",
    ] {
        assert_eq!(report.counts[category], 1, "{category}");
    }
}

#[test]
fn cli_json_is_deterministic_and_rejects_bad_arguments() {
    let run = || {
        std::process::Command::new(env!("CARGO_BIN_EXE_knowledge-audit"))
            .args(["--char", "嗯", "--json", "--check"])
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
    let doc: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(doc["character"], "嗯");
    assert_eq!(doc["resolved_full_codes"].as_array().unwrap().len(), 4);
    for args in [
        vec!["--char", "甲乙"],
        vec!["--char"],
        vec!["--unknown"],
        vec!["--json", "--export-tsv"],
    ] {
        assert!(
            !std::process::Command::new(env!("CARGO_BIN_EXE_knowledge-audit"))
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
}
