//! 便携 Rime 源包多产物生成的集成测试:仅检查内存中的生成结果,
//! 不使用 YAML 解析器,也不读取任何既有 Rime 词典。

use xhup_generator::generate_rime_artifacts;

/// 按文件名取产物内容。
fn contents_of<'a>(artifacts: &'a [xhup_generator::RimeArtifact], filename: &str) -> &'a str {
    artifacts
        .iter()
        .find(|artifact| artifact.filename() == filename)
        .unwrap_or_else(|| panic!("缺少产物 {filename}"))
        .contents()
}

#[test]
fn top_dictionary_imports_all_layer_dictionaries() {
    let artifacts = generate_rime_artifacts().unwrap();
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
        "  - xhup_flow_fixed_first_shortcuts",
        "  - xhup_flow_words",
    ] {
        assert!(dict.contains(line), "顶层词典缺少 `{line}`");
    }
    assert!(!dict.contains("{{"), "顶层词典存在未解析占位符");
}

#[test]
fn static_fallback_schema_semantics() {
    let artifacts = generate_rime_artifacts().unwrap();
    let schema = contents_of(&artifacts, "xhup_flow_static.schema.yaml");
    assert!(
        schema.contains("  schema_id: xhup_flow_static"),
        "静态兼容方案 schema_id 应为 xhup_flow_static"
    );
    // 与主方案相同的静态 translator 链,不含任何 Flow/学习 translator。
    assert!(
        schema.contains("  translators:\n    - punct_translator\n    - table_translator"),
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
        schema.contains("  dictionary: xhup_flow\n"),
        "静态兼容方案应复用主方案词典"
    );
}

#[test]
fn generation_is_byte_reproducible() {
    let first = generate_rime_artifacts().unwrap();
    let second = generate_rime_artifacts().unwrap();
    let first: Vec<(&str, &str)> = first.iter().map(|a| (a.filename(), a.contents())).collect();
    let second: Vec<(&str, &str)> = second
        .iter()
        .map(|a| (a.filename(), a.contents()))
        .collect();
    assert_eq!(first, second, "产物集合顺序与内容字节级一致");
}

#[test]
fn auxiliary_dictionaries_are_in_dependency_compile_graph() {
    // RC 回归守卫(#43 真机缺陷):librime 部署只编译默认 translator
    // 命名空间的词典;主方案引用的每一个非默认词典都必须
    // (a) 出现在主方案 `schema/dependencies`,且
    // (b) 存在同名 wrapper schema(schema_id 与 translator dictionary
    //     都等于该词典名),部署器才会为其生成 table.bin。
    // 缺任一项 => FIXED_FIRST/Flow/Learn 在真机静默失效。
    let artifacts = generate_rime_artifacts().unwrap();
    let schema = contents_of(&artifacts, "xhup_flow.schema.yaml");

    // 收集主方案引用的全部词典(所有 `dictionary: X` 行)。
    let mut referenced: Vec<&str> = Vec::new();
    for line in schema.lines() {
        let trimmed = line.trim();
        if let Some(dict) = trimmed.strip_prefix("dictionary: ") {
            let dict = dict.trim();
            if !referenced.contains(&dict) {
                referenced.push(dict);
            }
        }
    }
    assert!(referenced.contains(&"xhup_flow"), "主方案必须引用主词典");

    // 主方案的 dependencies 列表。
    let deps_start = schema
        .find("  dependencies:")
        .expect("主方案必须声明 schema/dependencies");
    let deps_block = &schema[deps_start..];
    let deps_end = deps_block
        .find(
            "
  description:",
        )
        .or_else(|| {
            deps_block.find(
                "

",
            )
        })
        .expect("dependencies 块应有边界");
    let deps_block = &deps_block[..deps_end];
    let dependencies: Vec<&str> = deps_block
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- "))
        .map(str::trim)
        .collect();

    for dict in referenced.iter().filter(|d| **d != "xhup_flow") {
        assert!(
            dependencies.contains(dict),
            "辅助词典 {dict} 未列入主方案 schema/dependencies:部署将静默失效"
        );
        let wrapper = artifacts
            .iter()
            .find(|a| a.filename() == format!("{dict}.schema.yaml"))
            .unwrap_or_else(|| panic!("辅助词典 {dict} 缺少同名编译 wrapper schema"));
        assert!(
            wrapper
                .contents()
                .contains(&format!("schema_id: \"{dict}\""))
                || wrapper.contents().contains(&format!("schema_id: {dict}")),
            "wrapper schema id 必须等于词典名 {dict}"
        );
        assert!(
            wrapper.contents().contains(&format!("dictionary: {dict}")),
            "wrapper 必须把默认命名空间词典指向 {dict}"
        );
    }
}

#[test]
fn artifact_manifest_forbids_user_owned_paths() {
    // 共享配置的唯一例外是用户明确授权的独占列表；精确内容另行固定。
    // 原始共享配置必须由安装器备份恢复，包中不能带私人备份或其他用户状态。
    const FORBIDDEN_EXACT: &[&str] = &[
        ".xhup-flow-default-backup.json",
        "default.yaml",
        "installation.yaml",
        "user.yaml",
        "rime.lua",
    ];
    let artifacts = generate_rime_artifacts().unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for artifact in &artifacts {
        let name = artifact.filename();
        assert!(seen.insert(name), "产物文件名必须唯一: {name}");
        assert!(!FORBIDDEN_EXACT.contains(&name), "禁止产物: {name}");
        assert!(!name.contains(".."), "产物路径不得含 `..`: {name}");
        assert!(!name.starts_with('/'), "产物路径不得为绝对路径: {name}");
        assert!(!name.contains('\\'), "产物路径只用 `/` 分隔: {name}");
        assert!(
            !name.contains("/sync/") && !name.starts_with("sync/"),
            "不得含 sync/: {name}"
        );
        assert!(!name.starts_with("build/"), "不得含 build/: {name}");
        assert!(!name.contains(".userdb"), "不得含 userdb: {name}");
        // 子目录产物只能进入本包拥有的 Lua 命名空间或许可证目录。
        if let Some((dir, _)) = name.split_once('/') {
            assert!(
                name.starts_with("lua/xhup_flow/") || name.starts_with("licenses/"),
                "产物不得写入其他用户目录: {name}(实际目录 {dir})"
            );
        }
    }
}
