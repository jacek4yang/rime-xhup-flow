use serde_json::Value;
use xhup_analyzer::{CodeOccupancy, ReplayCostModel, Replayer};
use xhup_core::{InputHanzi, XhupHanzi};
use xhup_generator::{
    canonical_extended_word_code_entries, canonical_fixed_first_shortcut_entries,
    canonical_input_char_code_entries, canonical_primary_shortcut_entries,
    canonical_word_code_entries, generate_rime_artifacts,
};

const SNAPSHOT: &str = include_str!("../../../data/benchmarks/v1-static-baseline.json");
const REPLAY_FIXTURE: &str = include_str!("../../../data/corpus/replay_fixture.txt");
const ATTESTED_CODES: &str = include_str!("../../../data/xhup/attested_char_codes.tsv");

fn snapshot() -> Value {
    serde_json::from_str(SNAPSHOT).expect("v1 baseline JSON 应合法")
}

fn number(doc: &Value, pointer: &str) -> u64 {
    doc.pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("v1 baseline 缺少整数 {pointer}"))
}

#[test]
fn canonical_counts_match_v1_release_snapshot() {
    let doc = snapshot();
    assert_eq!(doc["schema"], "xhup-v1-static-baseline/v1");
    assert_eq!(
        XhupHanzi::all().len() as u64,
        number(&doc, "/characters/coreStandardHanzi")
    );
    assert_eq!(
        InputHanzi::all().len() as u64,
        number(&doc, "/characters/inputHanzi")
    );
    assert_eq!(
        ATTESTED_CODES
            .lines()
            .filter(|line| !line.starts_with('#'))
            .count() as u64,
        number(&doc, "/characters/attestedEvidenceRows")
    );
    let char_entries = canonical_input_char_code_entries();
    assert_eq!(
        char_entries.len() as u64,
        number(&doc, "/characters/productionCodeRelations")
    );
    for (length, pointer) in [
        (2, "/characters/twoKeyRelations"),
        (3, "/characters/threeKeyRelations"),
        (4, "/characters/fourKeyRelations"),
    ] {
        assert_eq!(
            char_entries
                .iter()
                .filter(|entry| entry.code().len() == length)
                .count() as u64,
            number(&doc, pointer)
        );
    }
    assert_eq!(
        canonical_word_code_entries().len() as u64,
        number(&doc, "/lexicon/hotSemanticEntries")
    );
    assert_eq!(
        canonical_extended_word_code_entries().len() as u64,
        number(&doc, "/lexicon/extendedSemanticEntries")
    );
    assert_eq!(
        canonical_primary_shortcut_entries().len() as u64,
        number(&doc, "/static/primaryMappings")
    );
    assert_eq!(
        canonical_fixed_first_shortcut_entries().len() as u64,
        number(&doc, "/static/fixedFirstMappings")
    );
    assert_eq!(
        (canonical_primary_shortcut_entries().len()
            + canonical_fixed_first_shortcut_entries().len()) as u64,
        number(&doc, "/static/totalCompilerMappings")
    );
    assert_eq!(
        CodeOccupancy::build_current_production()
            .occupied_codes()
            .count() as u64,
        number(&doc, "/static/distinctExactCodes")
    );
}

#[test]
fn generated_package_bytes_match_v1_release_snapshot() {
    let doc = snapshot();
    let artifacts = generate_rime_artifacts();
    assert_eq!(
        artifacts.len() as u64,
        number(&doc, "/generatedRimePackage/artifactCount")
    );
    let bytes: usize = artifacts
        .iter()
        .map(|artifact| artifact.contents().len())
        .sum();
    assert_eq!(
        bytes as u64,
        number(&doc, "/generatedRimePackage/sourceBytes")
    );
}

#[test]
fn replay_totals_match_v1_release_snapshot() {
    let doc = snapshot();
    let report = Replayer::new(&ReplayCostModel::default()).replay_corpus(REPLAY_FIXTURE.lines());
    assert_eq!(report.sentences, number(&doc, "/replay/sentences"));
    assert_eq!(
        report.totals.chars as u64,
        number(&doc, "/replay/characters")
    );
    assert_eq!(report.totals.tokens as u64, number(&doc, "/replay/tokens"));
    assert_eq!(report.totals.keys as u64, number(&doc, "/replay/keys"));
    assert_eq!(
        report.totals.rank1 as u64,
        number(&doc, "/replay/rank1Tokens")
    );
    assert_eq!(
        report.totals.top3 as u64,
        number(&doc, "/replay/top3Tokens")
    );
    assert_eq!(
        report.totals.fallback_tokens as u64,
        number(&doc, "/replay/fallbackTokens")
    );
}
