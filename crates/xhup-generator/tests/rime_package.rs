//! 便携 Rime 源包多产物生成的集成测试:仅检查内存中的生成结果,
//! 不使用 YAML 解析器,也不读取任何既有 Rime 词典。

use xhup_generator::{
    generate_rime_artifacts, generate_rime_char_dictionary, generate_rime_word_dictionary,
};

/// 按文件名取产物内容。
fn contents_of<'a>(artifacts: &'a [xhup_generator::RimeArtifact], filename: &str) -> &'a str {
    artifacts
        .iter()
        .find(|artifact| artifact.filename() == filename)
        .unwrap_or_else(|| panic!("缺少产物 {filename}"))
        .contents()
}

#[test]
fn artifact_set_is_exact_and_ordered() {
    let artifacts = generate_rime_artifacts();
    let filenames: Vec<&str> = artifacts.iter().map(|a| a.filename()).collect();
    assert_eq!(
        filenames,
        [
            "xhup_flow_chars.dict.yaml",
            "xhup_flow_words.dict.yaml",
            "xhup_flow.dict.yaml",
            "xhup_flow.schema.yaml",
        ],
        "产物集合与顺序固定:单字词典 → 词语词典 → 顶层词典 → 方案"
    );
}

#[test]
fn char_dictionary_reuses_existing_generator() {
    let artifacts = generate_rime_artifacts();
    assert_eq!(
        contents_of(&artifacts, "xhup_flow_chars.dict.yaml"),
        generate_rime_char_dictionary(),
        "单字词典产物与既有生成器字节一致"
    );
}

#[test]
fn word_dictionary_reuses_existing_generator() {
    let artifacts = generate_rime_artifacts();
    assert_eq!(
        contents_of(&artifacts, "xhup_flow_words.dict.yaml"),
        generate_rime_word_dictionary(),
        "词语词典产物与既有生成器字节一致"
    );
}

#[test]
fn top_dictionary_imports_char_and_word_dictionaries() {
    let artifacts = generate_rime_artifacts();
    let dict = contents_of(&artifacts, "xhup_flow.dict.yaml");
    let expected_version = format!("version: \"{}\"", env!("CARGO_PKG_VERSION"));
    for line in [
        "name: xhup_flow",
        expected_version.as_str(),
        "sort: by_weight",
        "use_preset_vocabulary: false",
        "import_tables:",
        "  - xhup_flow_chars",
        "  - xhup_flow_words",
    ] {
        assert!(dict.contains(line), "顶层词典缺少 `{line}`");
    }
    assert!(!dict.contains("{{"), "顶层词典存在未解析占位符");
}

#[test]
fn schema_semantics() {
    let artifacts = generate_rime_artifacts();
    let schema = contents_of(&artifacts, "xhup_flow.schema.yaml");
    let expected_version = format!("  version: \"{}\"", env!("CARGO_PKG_VERSION"));
    for line in [
        "  schema_id: xhup_flow",
        "  name: 小鹤音形",
        expected_version.as_str(),
        "    - table_translator",
        "    - navigator",
        "  alphabet: zyxwvutsrqponmlkjihgfedcba",
        "  dictionary: xhup_flow",
        "  enable_completion: false",
        "  enable_sentence: false",
        "  enable_user_dict: false",
        "  import_preset: default",
    ] {
        assert!(schema.contains(line), "方案缺少 `{line}`");
    }
}

#[test]
fn schema_excludes_non_portable_or_deferred_features() {
    let artifacts = generate_rime_artifacts();
    let schema = contents_of(&artifacts, "xhup_flow.schema.yaml");
    for forbidden in [
        "script_translator",
        "recognizer",
        "matcher",
        "algebra",
        "delimiter",
        "lua",
        "opencc",
        "predict",
        "octagram",
        "simplifier",
        "enable_encoder",
        "filters:",
        "/",
    ] {
        assert!(!schema.contains(forbidden), "方案不应包含 `{forbidden}`");
    }
}

#[test]
fn all_artifacts_are_lf_only_with_single_final_newline() {
    for artifact in generate_rime_artifacts() {
        let text = artifact.contents();
        assert!(
            !text.starts_with('\u{feff}'),
            "{}: 无 BOM",
            artifact.filename()
        );
        assert!(!text.contains('\r'), "{}: LF only", artifact.filename());
        assert!(
            text.ends_with('\n') && !text.ends_with("\n\n"),
            "{}: 恰好一个末尾换行",
            artifact.filename()
        );
    }
}

#[test]
fn generation_is_byte_reproducible() {
    let first = generate_rime_artifacts();
    let second = generate_rime_artifacts();
    let first: Vec<(&str, &str)> = first.iter().map(|a| (a.filename(), a.contents())).collect();
    let second: Vec<(&str, &str)> = second
        .iter()
        .map(|a| (a.filename(), a.contents()))
        .collect();
    assert_eq!(first, second, "产物集合顺序与内容字节级一致");
}
