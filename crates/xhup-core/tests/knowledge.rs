use xhup_core::knowledge::*;

const ROW: &str = "甲\tjia\tprimary";
fn fixture() -> String {
    format!("# xhup-knowledge-evidence/v1\nreading\t{ROW}\tcore-readings\tattested\thigh\n")
}

#[test]
fn normalized_parser_roundtrip_and_invalid_rows() {
    let text = fixture();
    let parsed = parse_evidence(&text).unwrap();
    assert_eq!(serialize_evidence(&parsed).unwrap(), text);
    for bad in [
        text.replace("/v1", "/v2"),
        text.replace("甲", "甲乙"),
        text.replace("jia", "JiA"),
        text.replace("primary", "main"),
        text.replace("high", "certain"),
        text.replace("attested", "guessed"),
        text.replace('\n', "\r\n"),
        text.trim_end().into(),
        format!("\u{feff}{text}"),
        text.replace("甲", " "),
    ] {
        assert!(parse_evidence(&bad).is_err(), "{bad:?}");
    }
    assert!("\u{20000}".parse::<CharacterId>().is_ok());
    assert!("U+D800".parse::<CharacterId>().is_err());
    assert!("甲\u{fe00}".parse::<CharacterId>().is_err());
}

#[test]
fn all_relation_types_roundtrip_without_reading_invention() {
    let text = concat!(
        "# xhup-knowledge-evidence/v1\n",
        "frequency\t嗯\t12\tn\tcore-readings\tattested\tmedium\n",
        "full\t嗯\tonkx\t0\tflypy-official-ix\tofficial\thigh\n",
        "generated\t甲\tjxop\tjia\tflow-core-composition\tgenerated\thigh\n",
        "reading\t嗯\tn\tprimary\tcore-readings\tattested\thigh\n",
        "shape\t嗯\tkx\t-\trime-fast-xhup\tattested\thigh\n",
        "sound\t嗯\ton\t-\tflypy-official-ix\tofficial\thigh\n",
        "variant\t甲\t乙\tother\tcore-readings\tuncertain\tlow\n"
    );
    let e = parse_evidence(text).unwrap();
    assert_eq!(serialize_evidence(&e).unwrap(), text);
    assert_eq!(e.readings[0].reading, "n");
    assert_eq!(e.sounds[0].code.to_string(), "on");
}

#[test]
fn duplicate_semantic_rows_rejected_even_if_status_or_weight_differs() {
    let text = fixture();
    let row = text.lines().nth(1).unwrap();
    assert!(parse_evidence(&format!("{text}{row}\n")).is_err());
    let mut evidence = parse_evidence(&text).unwrap();
    let mut duplicate = evidence.readings[0];
    duplicate.provenance.status = EvidenceStatus::Uncertain;
    evidence.readings.push(duplicate);
    assert!(serialize_evidence(&evidence).is_err());
    assert!(parse_full_codes("甲\tjx\top\t1\trime-fast-xhup\tattested\n甲\tjx\top\t2\trime-fast-xhup\tlegacy-compatible\n").is_err());
}

#[test]
fn source_parser_rejects_orphans_license_drift_and_machine_paths() {
    let text = serialize_sources(canonical_sources()).unwrap();
    assert_eq!(
        serialize_sources(&parse_sources(&text).unwrap()).unwrap(),
        text
    );
    for bad in [
        text.replace("MIT", "unknown"),
        text.replace("redistributable", "allowed"),
        text.replace("kMandarin_8105.txt+kTGHZ2013.txt", "C:/private/file"),
        text.replace("923b108dc5d45dee061324c011b478fb649f8b73", "main"),
        text.replace("3c2773335c8108bbe896b9af588d619e22045132", "bad-hash"),
        format!("{text}{}\n", text.lines().nth(1).unwrap()),
    ] {
        assert!(parse_sources(&bad).is_err());
    }
    let e = parse_full_codes("甲\tjx\top\t1\tmissing\tattested\n").unwrap();
    assert!(
        resolve_full_codes(&e, canonical_sources())
            .unwrap_err()
            .to_string()
            .contains("orphan")
    );
}

#[test]
fn source_precedence_is_deterministic_and_preserves_conflicting_aliases() {
    let text = "嗯\ten\tkx\t0\tflypy-official-ix\tofficial-yield-full\n嗯\ten\tkx\t46\trime-fast-xhup\tlegacy-compatible\n嗯\tng\tkx\t46\trime-fast-xhup\tlegacy-compatible\n";
    let mut e = parse_full_codes(text).unwrap();
    let first = resolve_full_codes(&e, canonical_sources()).unwrap();
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].preferred.source, "flypy-official-ix");
    assert_eq!(first[0].supporting.len(), 2);
    e.reverse();
    assert_eq!(resolve_full_codes(&e, canonical_sources()).unwrap(), first);
    assert_eq!(serialize_full_codes(&e).unwrap(), text);
    e[0].provenance.status = EvidenceStatus::Deprecated;
    assert_eq!(
        resolve_full_codes(&e, canonical_sources()).unwrap().len(),
        1
    );
}

#[test]
fn valid_synthetic_evidence_extends_production_resolution_without_ceiling() {
    let mut evidence =
        parse_full_codes(include_str!("../../../data/xhup/attested_char_codes.tsv")).unwrap();
    let before = resolve_full_codes(&evidence, canonical_sources()).unwrap();
    let synthetic = CharacterId('\u{10ffff}');
    assert!(!before.iter().any(|r| r.character == synthetic));
    let mut row = evidence[0];
    row.character = synthetic;
    evidence.push(row);
    let after = resolve_full_codes(&evidence, canonical_sources()).unwrap();
    let members = |r: &[ResolvedFullCode<'_>]| {
        r.iter()
            .map(|r| r.character)
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(members(&after).len(), members(&before).len() + 1);
    assert!(members(&after).contains(&synthetic));
}

#[test]
fn equal_status_uses_confidence_then_source_priority_then_id() {
    let mut a = canonical_sources()
        .iter()
        .find(|s| s.id == "rime-fast-xhup")
        .unwrap()
        .clone();
    a.id = "a";
    a.priority = 1;
    let mut b = a.clone();
    b.id = "b";
    b.priority = 2;
    let sources = [a, b];
    let mut e =
        parse_full_codes("甲\taa\tbb\t1\ta\tattested\n甲\taa\tbb\t1\tb\tattested\n").unwrap();
    assert_eq!(
        resolve_full_codes(&e, &sources).unwrap()[0]
            .preferred
            .source,
        "b"
    );
    e[1].provenance.confidence = EvidenceConfidence::Medium;
    assert_eq!(
        resolve_full_codes(&e, &sources).unwrap()[0]
            .preferred
            .source,
        "a"
    );
    e[0].provenance.confidence = EvidenceConfidence::Low;
    assert_eq!(
        resolve_full_codes(&e, &sources).unwrap()[0]
            .supporting
            .len(),
        1
    );
    e[1].provenance.status = EvidenceStatus::Uncertain;
    assert!(resolve_full_codes(&e, &sources).unwrap().is_empty());
}
