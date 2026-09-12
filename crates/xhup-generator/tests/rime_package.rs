//! 便携 Rime 源包多产物生成的集成测试:仅检查内存中的生成结果,
//! 不使用 YAML 解析器,也不读取任何既有 Rime 词典。

use xhup_generator::{
    generate_rime_artifacts, generate_rime_char_dictionary,
    generate_rime_fixed_first_shortcut_dictionary, generate_rime_shortcut_dictionary,
    generate_rime_word_dictionary, generate_rime_word_shortcut_dictionary,
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
            "xhup_flow_words.dict.yaml",
            "xhup_flow.dict.yaml",
            "xhup_flow_fixed_first_shortcuts.dict.yaml",
            "xhup_flow_flow.dict.yaml",
            "xhup_flow_learn.dict.yaml",
            "xhup_flow_flow.schema.yaml",
            "xhup_flow_learn.schema.yaml",
            "lua/xhup_flow/annotation.lua",
            "lua/xhup_flow/quick_hint.lua",
            "lua/xhup_flow/init.lua",
            "lua/xhup_flow/data/quick_hints.lua",
            "xhup_flow.schema.yaml",
            "xhup_flow_static.schema.yaml",
        ],
        "产物集合与顺序固定:一级简码 → 单字 → PRIMARY → 固定词 → 顶层词典 → FIXED_FIRST → Flow/Learn → 两个编译 wrapper → Lua → 两套方案"
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
        "  - xhup_flow_fixed_first_shortcuts",
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
        "    - navigator",
        "  alphabet: zyxwvutsrqponmlkjihgfedcba",
        "  import_preset: default",
    ] {
        assert!(schema.contains(line), "方案缺少 `{line}`");
    }
    // translator 链:全部静态层在唯一 primary table translator 中。
    assert!(
        schema.contains(
            "  translators:\n    - punct_translator\n    - table_translator\n    - table_translator@flow\n    - table_translator@learn"
        ),
        "translator 链应为 punct → static primary → Flow"
    );
    // primary translator:全部既有固定层;initial_quality 1000000 只是
    // translator 间优先级栅栏,不改变其内部相对次序。
    assert!(
        schema.contains(
            "translator:\n  dictionary: xhup_flow\n  enable_completion: false\n  enable_sentence: false\n  enable_user_dict: false\n  initial_quality: 1000000"
        ),
        "primary translator 配置不符合冻结语义"
    );
    assert!(
        !schema.contains("table_translator@fixed_first") && !schema.contains("fixed_first:"),
        "FIXED_FIRST 必须并入同一静态 translator"
    );
    // Flow translator:完整词汇 + 单字开放组句 + 共享用户词典；严格位于静态之后。
    // 无自动提交、无 completion。
    assert!(
        schema.contains(
            "flow:\n  dictionary: xhup_flow_flow\n  user_dict: xhup_flow_user\n  enable_completion: false\n  enable_sentence: true\n  sentence_over_completion: true\n  enable_user_dict: true\n  initial_quality: 100"
        ),
        "flow translator 配置不符合组句语义"
    );
    // learn translator:学习短语编码(encoder),关闭组句,共享用户词典。
    assert!(
        schema.contains(
            "learn:\n  dictionary: xhup_flow_learn\n  user_dict: xhup_flow_user\n  enable_completion: false\n  enable_sentence: false\n  enable_user_dict: true\n  enable_encoder: true\n  encode_commit_history: true\n  max_phrase_length: 20\n  max_homographs: 1\n  initial_quality: 1000"
        ),
        "learn translator 配置不符合学习语义"
    );
    // uniquifier:同一词在静态与动态层重合时去重,动态候选只追加在后。
    // Lua 简码提示:filters 链 quick_hint → uniquifier(注解类 filter 在
    // 前,text 级去重兜底在最后,见 schema 模板注释);开关默认开(reset 1)。
    let filter_entries: Vec<&str> = schema
        .lines()
        .skip_while(|line| *line != "  filters:")
        .skip(1)
        .take_while(|line| line.starts_with("    "))
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect();
    assert_eq!(
        filter_entries,
        ["    - lua_filter@*xhup_flow.quick_hint", "    - uniquifier"],
        "filters 链应为 quick_hint → uniquifier"
    );
    assert!(
        schema.contains("- name: quick_hint\n    reset: 1"),
        "quick_hint 开关应默认开启"
    );
    assert!(
        schema.contains("- name: debug_candidate_annotations\n    reset: 0"),
        "debug_candidate_annotations 开关应默认关闭"
    );
    // Lua 产物:模块源码与生成数据俱在,数据与 canonical 简码映射一致。
    let annotation = contents_of(&artifacts, "lua/xhup_flow/annotation.lua");
    assert!(annotation.contains("format_shortcut_hint"));
    let module = contents_of(&artifacts, "lua/xhup_flow/quick_hint.lua");
    assert!(module.contains("xhup_flow.data.quick_hints"));
    assert!(module.contains("xhup_flow.annotation"));
    let data = contents_of(&artifacts, "lua/xhup_flow/data/quick_hints.lua");
    assert!(
        data.contains("[\"时间\"] = \"uij\""),
        "提示数据应含 时间 → uij"
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
fn schema_excludes_non_portable_or_deferred_features() {
    let artifacts = generate_rime_artifacts();
    let schema = contents_of(&artifacts, "xhup_flow.schema.yaml");
    for forbidden in [
        "script_translator",
        "recognizer",
        "matcher",
        "algebra",
        "delimiter",
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
    // Lua 唯一许可例外:quick_hint 简码提示(2.0 mandatory Lua 合同组件);
    // 其余任何 lua 组件引用仍然禁止。
    let lua_lines: Vec<&str> = schema
        .lines()
        .filter(|line| line.contains("lua_") && !line.trim_start().starts_with('#'))
        .collect();
    assert_eq!(
        lua_lines,
        ["    - lua_filter@*xhup_flow.quick_hint"],
        "方案只允许 quick_hint 一个 Lua 组件引用"
    );
    // 行内斜杠只允许出现在注释与 quick_hint 模块路径中。
    for line in schema.lines() {
        let code = line.split('#').next().unwrap_or(line);
        if code.contains("lua_filter@*xhup_flow.quick_hint") {
            continue;
        }
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

#[test]
fn auxiliary_dictionaries_are_in_dependency_compile_graph() {
    // RC 回归守卫(#43 真机缺陷):librime 部署只编译默认 translator
    // 命名空间的词典;主方案引用的每一个非默认词典都必须
    // (a) 出现在主方案 `schema/dependencies`,且
    // (b) 存在同名 wrapper schema(schema_id 与 translator dictionary
    //     都等于该词典名),部署器才会为其生成 table.bin。
    // 缺任一项 => FIXED_FIRST/Flow/Learn 在真机静默失效。
    let artifacts = generate_rime_artifacts();
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
    // 产物清单安全不变量(发布级):XHUP 源包只做「自有文件 overlay」,
    // 绝不携带会覆盖用户状态/配置的文件名,也不携带路径逃逸。
    const FORBIDDEN_EXACT: &[&str] = &[
        "default.custom.yaml",
        "default.yaml",
        "installation.yaml",
        "user.yaml",
        "rime.lua",
    ];
    let artifacts = generate_rime_artifacts();
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
        // 子目录产物只允许 Lua 命名空间(安全不变量:用户 lua/ 下其它
        // 文件绝不被触及)。
        if let Some((dir, _)) = name.split_once('/') {
            assert!(
                name.starts_with("lua/xhup_flow/"),
                "子目录产物只允许 lua/xhup_flow/**: {name}(实际目录 {dir})"
            );
        }
    }
}
