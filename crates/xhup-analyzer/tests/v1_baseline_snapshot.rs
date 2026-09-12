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

const FROZEN_STATIC_ARTIFACTS: &[&str] = &[
    "lua/xhup_flow/data/quick_hints.lua",
    "xhup_flow.dict.yaml",
    "xhup_flow_chars.dict.yaml",
    "xhup_flow_fixed_first_shortcuts.dict.yaml",
    "xhup_flow_flow.dict.yaml",
    "xhup_flow_flow.schema.yaml",
    "xhup_flow_learn.dict.yaml",
    "xhup_flow_learn.schema.yaml",
    "xhup_flow_shortcuts.dict.yaml",
    "xhup_flow_static.schema.yaml",
    "xhup_flow_word_shortcuts.dict.yaml",
    "xhup_flow_words.dict.yaml",
    "xhup_flow_trainer.json",
];

#[test]
fn generated_package_bytes_match_v1_release_snapshot() {
    let doc = snapshot();
    // 区分冻结 v1 发布 oracle 与当前 Flow 2.0 产物演进:
    // v1 历史发布快照记录 14 个产物, 源码 36,845,122 字节:
    assert_eq!(
        number(&doc, "/generatedRimePackage/artifactCount"),
        14,
        "v1 历史发布快照记录 14 个产物"
    );
    assert_eq!(
        number(&doc, "/generatedRimePackage/sourceBytes"),
        36845122,
        "v1 历史发布快照记录 36845122 源码字节"
    );

    // 当前 2.0 便携包扩充 annotation.lua (共 15 个产物):
    let artifacts = generate_rime_artifacts();
    assert_eq!(artifacts.len(), 15, "2.0 包扩充候选注释格式化模块");

    // 冻结的 12 个静态模式与词典产物字节严格不变:
    let static_bytes: usize = artifacts
        .iter()
        .filter(|a| FROZEN_STATIC_ARTIFACTS.contains(&a.filename()))
        .map(|artifact| artifact.contents().len())
        .sum();
    assert_eq!(static_bytes, 36838387, "静态基线产物字节数保持恒定");
}

#[test]
fn artifact_content_matches_independent_v1_release_hashes() {
    use std::collections::BTreeMap;
    use xhup_analyzer::export_v2::sha256_hex;
    let actual: BTreeMap<_, _> = generate_rime_artifacts()
        .into_iter()
        .map(|artifact| {
            (
                artifact.filename().to_string(),
                sha256_hex(artifact.contents().as_bytes()),
            )
        })
        .chain(std::iter::once((
            "xhup_flow_trainer.json".into(),
            sha256_hex(xhup_generator::generate_trainer_dataset().as_bytes()),
        )))
        .collect();
    let mut expected = BTreeMap::new();
    let manifest = include_str!("../../../data/benchmarks/v1-artifact-hashes.tsv");
    assert!(manifest.starts_with("# xhup-v1-artifact-hashes/v1\n"));
    assert!(manifest.contains("# source: https://github.com/jacek4yang/rime-xhup-flow/releases/download/xhup-flow-v1.0.0/CANONICAL-SHA256SUMS.txt\n"));
    assert!(manifest.contains("# upstream manifest SHA-256: 3f1baa21104266f06973f47a5464f2a4d07dde2048fa3d2a5c6ecccee93a6558\n"));
    for line in manifest.lines().filter(|l| !l.starts_with('#')) {
        let (path, hash) = line
            .split_once('\t')
            .expect("strict two-column release fixture");
        assert_eq!(hash.len(), 64);
        assert!(
            hash.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        assert!(
            expected
                .insert(path.to_string(), hash.to_string())
                .is_none()
        );
    }

    // 提取不可变的 12 个静态方案/词典与 Trainer 数据产物
    let actual_frozen: BTreeMap<_, _> = actual
        .into_iter()
        .filter(|(k, _)| FROZEN_STATIC_ARTIFACTS.contains(&k.as_str()))
        .collect();
    let expected_frozen: BTreeMap<_, _> = expected
        .into_iter()
        .filter(|(k, _)| FROZEN_STATIC_ARTIFACTS.contains(&k.as_str()))
        .collect();

    assert_eq!(actual_frozen.len(), 13);
    assert_eq!(
        actual_frozen, expected_frozen,
        "frozen static artifacts must match the independently published v1 release"
    );
}

#[test]
fn runtime_lua_artifacts_meet_ascii_and_decoupled_contract() {
    let artifacts = generate_rime_artifacts();
    let annotation = artifacts
        .iter()
        .find(|a| a.filename() == "lua/xhup_flow/annotation.lua")
        .expect("annotation.lua 产物必须存在");
    let quick_hint = artifacts
        .iter()
        .find(|a| a.filename() == "lua/xhup_flow/quick_hint.lua")
        .expect("quick_hint.lua 产物必须存在");

    assert!(!annotation.contents().contains('\r'), "LF only");
    assert!(!quick_hint.contents().contains('\r'), "LF only");

    // 注释模块包含清洗与格式化入口
    assert!(annotation.contents().contains("clean_decorative_markers"));
    assert!(annotation.contents().contains("format_shortcut_hint"));
    assert!(annotation.contents().contains("format_candidate_comment"));
    assert!(annotation.contents().contains("debug_tag_for_type"));

    // 简码提示模块委托至 annotation 模块
    assert!(
        quick_hint
            .contents()
            .contains("require(\"xhup_flow.annotation\")")
    );
    assert!(
        quick_hint
            .contents()
            .contains("debug_candidate_annotations")
    );

    // 正常提示逻辑中不写死装饰 emoji
    assert!(!quick_hint.contents().contains("\"⚡\""));
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
