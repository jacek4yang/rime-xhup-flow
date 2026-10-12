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
    // 2026-09-14 所有者决策:搜狗细胞词库聚合层并入扩展词证据
    // (data/words/sogou/README.md)。PR-4 起生产子集排除 target 与
    // llm_review-remove,扩展聚合条目数仍以 v1 万象发布值为下限;
    // hot 等其余计数仍与 v1 快照严格相等。`xhup_flow_static` 不受影响。
    assert!(
        canonical_extended_word_code_entries().len() as u64
            >= number(&doc, "/lexicon/extendedSemanticEntries"),
        "扩展词聚合条目应不少于 v1 发布基线"
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

// Portable Lua is compiled rather than byte-frozen. The complete hint mapping
// is frozen independently by tests/lua/run_runtime_suite.py against v1 values.
const FROZEN_STATIC_ARTIFACTS: &[&str] = &[
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
fn artifact_content_matches_independent_v1_release_hashes() {
    use std::collections::BTreeMap;
    use xhup_analyzer::export_v2::sha256_hex;
    // 版本号与冻结内容语义分离:产物头部的 `version:` 行随当前
    // package version 渲染(rc1 ≠ v1.0.0),比哈希前归一到 v1 的版本
    // 行,保证哈希等价性只检验**冻结内容**(词表/映射/结构)本身。
    const V1_VERSION_LINE: &str = "1.0.0";
    let normalize_version = |filename: &str, contents: String| -> String {
        if !filename.ends_with(".yaml") {
            return contents;
        }
        let version_prefix = "version: ";
        let mut out = String::with_capacity(contents.len());
        for line in contents.lines() {
            let trimmed = line.trim_start();
            if let Some(stripped) = trimmed.strip_prefix(version_prefix)
                && stripped.starts_with('"')
            {
                // 保留原行前导空白,仅替换引号内版本字面量。
                let indent_len = line.len() - trimmed.len();
                let indent = &line[..indent_len];
                out.push_str(indent);
                out.push_str(version_prefix);
                out.push_str(&format!("\"{V1_VERSION_LINE}\""));
                out.push('\n');
                continue;
            }
            out.push_str(line);
            out.push('\n');
        }
        out
    };
    let actual: BTreeMap<_, _> = generate_rime_artifacts()
        .unwrap()
        .into_iter()
        .map(|artifact| {
            (
                artifact.filename().to_string(),
                sha256_hex(
                    normalize_version(artifact.filename(), artifact.contents().to_string())
                        .as_bytes(),
                ),
            )
        })
        .chain(std::iter::once((
            "xhup_flow_trainer.json".into(),
            // 训练器数据集头部嵌入当前 package version(rc1 ≠ v1.0.0),
            // 与 YAML `version:` 行同策略:比哈希前归一到 v1 版本,
            // 保证哈希等价性只检验冻结内容本身。
            sha256_hex(
                xhup_generator::generate_trainer_dataset()
                    .replace(env!("CARGO_PKG_VERSION"), V1_VERSION_LINE)
                    .as_bytes(),
            ),
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

    // 冻结方案/词典和 Trainer 数据；Lua 映射另做真实 VM 的全表值比较。
    let actual_frozen: BTreeMap<_, _> = actual
        .into_iter()
        .filter(|(k, _)| FROZEN_STATIC_ARTIFACTS.contains(&k.as_str()))
        .collect();
    let expected_frozen: BTreeMap<_, _> = expected
        .into_iter()
        .filter(|(k, _)| FROZEN_STATIC_ARTIFACTS.contains(&k.as_str()))
        .collect();

    // Flow 组句/变长投影独立演进且必须遵守 clean-v1;其它冻结产物
    // 仍须与 v1 独立发布逐字节一致。历史搜狗政策不再适用。
    let actual_frozen_except_flow: BTreeMap<_, _> = actual_frozen
        .iter()
        .filter(|(k, _)| *k != "xhup_flow_flow.dict.yaml")
        .collect();
    let expected_frozen_except_flow: BTreeMap<_, _> = expected_frozen
        .iter()
        .filter(|(k, _)| *k != "xhup_flow_flow.dict.yaml")
        .collect();
    assert_eq!(
        actual_frozen_except_flow, expected_frozen_except_flow,
        "flow 词典以外的冻结产物必须与 v1 独立发布一致"
    );
    assert_ne!(
        actual_frozen.get("xhup_flow_flow.dict.yaml"),
        expected_frozen.get("xhup_flow_flow.dict.yaml"),
        "flow 组句投影独立演进(非静态字节冻结契约)"
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
