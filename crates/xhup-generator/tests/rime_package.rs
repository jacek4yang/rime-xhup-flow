//! 便携 Rime 源包多产物生成的集成测试:仅检查内存中的生成结果,
//! 不使用 YAML 解析器,也不读取任何既有 Rime 词典。

use xhup_generator::generate_rime_artifacts;

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
