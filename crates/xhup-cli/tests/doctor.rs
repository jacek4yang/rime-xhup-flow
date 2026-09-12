//! `doctor` 诊断命令集成测试。

use std::fs;
use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};

use clap::Parser;
use xhup_cli::{Cli, CliError, run};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_user_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "xhup-cli-doctor-test-{}-{}",
        process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn doctor_succeeds_on_fresh_generated_rime_package() {
    let dir = temp_user_dir();
    let plugin_dir = temp_user_dir();
    fs::create_dir_all(&plugin_dir).unwrap();
    fs::write(plugin_dir.join("librime-lua.so"), "").unwrap();

    // 先生成完整 Rime 包
    let gen_cli = Cli::try_parse_from([
        "xhup-cli",
        "generate",
        "rime",
        "--output",
        dir.to_str().unwrap(),
    ])
    .unwrap();
    run(gen_cli).unwrap();

    // 运行 doctor 检查
    let doctor_cli = Cli::try_parse_from([
        "xhup-cli",
        "doctor",
        "--user-data-dir",
        dir.to_str().unwrap(),
        "--plugins-dir",
        plugin_dir.to_str().unwrap(),
    ])
    .unwrap();
    run(doctor_cli).unwrap();

    let _ = fs::remove_dir_all(&plugin_dir);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn doctor_fails_when_mandatory_lua_modules_missing() {
    let dir = temp_user_dir();
    let plugin_dir = temp_user_dir();
    fs::create_dir_all(&plugin_dir).unwrap();
    fs::write(plugin_dir.join("librime-lua.so"), "").unwrap();

    let gen_cli = Cli::try_parse_from([
        "xhup-cli",
        "generate",
        "rime",
        "--output",
        dir.to_str().unwrap(),
    ])
    .unwrap();
    run(gen_cli).unwrap();

    // 删除必须的 Lua 模块
    fs::remove_file(dir.join("lua/xhup_flow/init.lua")).unwrap();

    let doctor_cli = Cli::try_parse_from([
        "xhup-cli",
        "doctor",
        "--user-data-dir",
        dir.to_str().unwrap(),
        "--plugins-dir",
        plugin_dir.to_str().unwrap(),
    ])
    .unwrap();
    let err = run(doctor_cli).unwrap_err();
    match err {
        CliError::Doctor(_) => {}
        other => panic!("期望 CliError::Doctor, 实际 {other:?}"),
    }

    let _ = fs::remove_dir_all(&plugin_dir);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn doctor_fails_when_lua_plugin_missing_for_flow() {
    let dir = temp_user_dir();
    let empty_plugin_dir = temp_user_dir();
    fs::create_dir_all(&empty_plugin_dir).unwrap();

    let gen_cli = Cli::try_parse_from([
        "xhup-cli",
        "generate",
        "rime",
        "--output",
        dir.to_str().unwrap(),
    ])
    .unwrap();
    run(gen_cli).unwrap();

    // 显式指向无 librime-lua 插件的目录，主方案 xhup_flow 必须判定失败
    let doctor_cli = Cli::try_parse_from([
        "xhup-cli",
        "doctor",
        "--user-data-dir",
        dir.to_str().unwrap(),
        "--schema",
        "xhup_flow",
        "--plugins-dir",
        empty_plugin_dir.to_str().unwrap(),
    ])
    .unwrap();
    let err = run(doctor_cli).unwrap_err();
    match err {
        CliError::Doctor(_) => {}
        other => panic!("期望 CliError::Doctor, 实际 {other:?}"),
    }

    let _ = fs::remove_dir_all(&empty_plugin_dir);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn doctor_passes_static_fallback_without_lua() {
    let dir = temp_user_dir();
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("xhup_flow_static.schema.yaml"), "# static").unwrap();

    // 针对 xhup_flow_static 方案运行 doctor，无 Lua 插件亦应通过
    let doctor_cli = Cli::try_parse_from([
        "xhup-cli",
        "doctor",
        "--user-data-dir",
        dir.to_str().unwrap(),
        "--schema",
        "xhup_flow_static",
    ])
    .unwrap();
    run(doctor_cli).unwrap();

    let _ = fs::remove_dir_all(&dir);
}
