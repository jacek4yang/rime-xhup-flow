//! 便携 Rime 源包多产物生成的集成测试:仅检查内存中的生成结果,
//! 不使用 YAML 解析器,也不读取任何既有 Rime 词典。

use xhup_generator::{
    generate_rime_artifacts, generate_rime_char_dictionary,
    generate_rime_fixed_first_shortcut_dictionary, generate_rime_shortcut_dictionary,
    generate_rime_two_key_shortcut_dictionary, generate_rime_word_dictionary,
    generate_rime_word_shortcut_dictionary,
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
            "xhup_flow_shortcuts.dict.yaml",
            "xhup_flow_chars.dict.yaml",
            "xhup_flow_word_shortcuts.dict.yaml",
            "xhup_flow_two_key_shortcuts.dict.yaml",
            "xhup_flow_words.dict.yaml",
            "xhup_flow.dict.yaml",
            "xhup_flow_fixed_first_shortcuts.dict.yaml",
            "xhup_flow_flow.dict.yaml",
            "xhup_flow_learn.dict.yaml",
            "xhup_flow.schema.yaml",
            "xhup_flow_static.schema.yaml",
        ],
        "产物集合与顺序固定:简码词典 → 单字词典 → 词语简码词典 → 二码简码词典 → 词语词典 → 顶层词典 → FIXED_FIRST 简码词典 → Flow 组句词典 → Flow 学习词典 → 方案 → 静态兼容方案"
    );
}

#[test]
fn shortcut_dictionary_reuses_existing_generator() {
    let artifacts = generate_rime_artifacts();
    assert_eq!(
        contents_of(&artifacts, "xhup_flow_shortcuts.dict.yaml"),
        generate_rime_shortcut_dictionary(),
        "一级简码词典产物与既有生成器字节一致"
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
fn word_shortcut_dictionary_reuses_existing_generator() {
    let artifacts = generate_rime_artifacts();
    assert_eq!(
        contents_of(&artifacts, "xhup_flow_word_shortcuts.dict.yaml"),
        generate_rime_word_shortcut_dictionary(),
        "词语简码词典产物与既有生成器字节一致"
    );
}

#[test]
fn two_key_shortcut_dictionary_reuses_existing_generator() {
    let artifacts = generate_rime_artifacts();
    assert_eq!(
        contents_of(&artifacts, "xhup_flow_two_key_shortcuts.dict.yaml"),
        generate_rime_two_key_shortcut_dictionary(),
        "二码零冲突简码词典产物与既有生成器字节一致"
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
fn fixed_first_shortcut_dictionary_reuses_existing_generator() {
    let artifacts = generate_rime_artifacts();
    assert_eq!(
        contents_of(&artifacts, "xhup_flow_fixed_first_shortcuts.dict.yaml"),
        generate_rime_fixed_first_shortcut_dictionary(),
        "FIXED_FIRST 词语简码词典产物与既有生成器字节一致"
    );
}

#[test]
fn top_dictionary_imports_all_layer_dictionaries() {
    let artifacts = generate_rime_artifacts();
    let dict = contents_of(&artifacts, "xhup_flow.dict.yaml");
    let expected_version = format!("version: \"{}\"", env!("CARGO_PKG_VERSION"));
    for line in [
        "name: xhup_flow",
        expected_version.as_str(),
        "sort: by_weight",
        "use_preset_vocabulary: false",
        "import_tables:",
        "  - xhup_flow_shortcuts",
        "  - xhup_flow_chars",
        "  - xhup_flow_word_shortcuts",
        "  - xhup_flow_two_key_shortcuts",
        "  - xhup_flow_words",
    ] {
        assert!(dict.contains(line), "顶层词典缺少 `{line}`");
    }
    // FIXED_FIRST 简码词典由方案中独立的第二 table_translator 加载,
    // 绝不导入顶层词典(否则无法保证既有固定候选次序不变)。
    assert!(
        !dict.contains("xhup_flow_fixed_first_shortcuts"),
        "顶层词典不得导入 FIXED_FIRST 简码词典"
    );
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
        "    - navigator",
        "  alphabet: zyxwvutsrqponmlkjihgfedcba",
        "  import_preset: default",
    ] {
        assert!(schema.contains(line), "方案缺少 `{line}`");
    }
    // translator 链:punct + primary table + 独立 FIXED_FIRST table,
    // 顺序与命名空间精确锁定。
    assert!(
        schema.contains(
            "  translators:\n    - punct_translator\n    - table_translator\n    - table_translator@fixed_first"
        ),
        "translator 链应为 punct_translator → table_translator → table_translator@fixed_first"
    );
    // primary translator:全部既有固定层;initial_quality 1000000 只是
    // translator 间优先级栅栏,不改变其内部相对次序。
    assert!(
        schema.contains(
            "translator:\n  dictionary: xhup_flow\n  enable_completion: false\n  enable_sentence: false\n  enable_user_dict: false\n  initial_quality: 1000000"
        ),
        "primary translator 配置不符合冻结语义"
    );
    // FIXED_FIRST translator:独立静态词典,initial_quality 0 严格靠后。
    assert!(
        schema.contains(
            "fixed_first:\n  dictionary: xhup_flow_fixed_first_shortcuts\n  enable_completion: false\n  enable_sentence: false\n  enable_user_dict: false\n  initial_quality: 0"
        ),
        "fixed_first translator 配置不符合冻结语义"
    );
    // Flow translator:连续组句 + 共享用户词典,initial_quality 0 严格靠后;
    // 无自动提交、无 completion。
    assert!(
        schema.contains(
            "flow:\n  dictionary: xhup_flow_flow\n  user_dict: xhup_flow_user\n  enable_completion: false\n  enable_sentence: true\n  sentence_over_completion: true\n  enable_user_dict: true\n  initial_quality: 0"
        ),
        "flow translator 配置不符合组句语义"
    );
    // learn translator:学习短语编码(encoder),关闭组句,共享用户词典。
    assert!(
        schema.contains(
            "learn:\n  dictionary: xhup_flow_learn\n  user_dict: xhup_flow_user\n  enable_completion: false\n  enable_sentence: false\n  enable_user_dict: true\n  enable_encoder: true\n  encode_commit_history: true\n  max_phrase_length: 20\n  max_homographs: 1\n  initial_quality: 0"
        ),
        "learn translator 配置不符合学习语义"
    );
    // uniquifier:同一词在静态与动态层重合时去重,动态候选只追加在后。
    assert!(
        schema.contains("filters:\n    - uniquifier"),
        "方案应含 uniquifier 过滤器"
    );
}

#[test]
fn static_fallback_schema_semantics() {
    let artifacts = generate_rime_artifacts();
    let schema = contents_of(&artifacts, "xhup_flow_static.schema.yaml");
    assert!(
        schema.contains("  schema_id: xhup_flow_static"),
        "静态兼容方案 schema_id 应为 xhup_flow_static"
    );
    // 与主方案相同的静态 translator 链,不含任何 Flow/学习 translator。
    assert!(
        schema.contains(
            "  translators:\n    - punct_translator\n    - table_translator\n    - table_translator@fixed_first"
        ),
        "静态兼容方案 translator 链应与主方案静态部分一致"
    );
    for forbidden in [
        "@flow",
        "@learn",
        "xhup_flow_user",
        "uniquifier",
        "filters:",
    ] {
        assert!(
            !schema.contains(forbidden),
            "静态兼容方案不应包含 `{forbidden}`"
        );
    }
    // 复用同一组静态词典,不重复数据。
    assert!(
        schema.contains("  dictionary: xhup_flow\n")
            && schema.contains("  dictionary: xhup_flow_fixed_first_shortcuts"),
        "静态兼容方案应复用主方案词典"
    );
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
        "auto_select",
        "auto_commit",
        "enable_completion: true",
    ] {
        assert!(!schema.contains(forbidden), "方案不应包含 `{forbidden}`");
    }
    // 行内斜杠只允许出现在注释中(引擎配置值不含 `/`)。
    for line in schema.lines() {
        let code = line.split('#').next().unwrap_or(line);
        assert!(!code.contains('/'), "方案非注释配置不应包含 `/`: {line}");
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
