//! `generate rime` 文件系统编排测试(不经子进程,不写仓库 dist/)。

use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};

use clap::Parser;
use xhup_cli::{Cli, CliError, run};
use xhup_generator::{RIME_CHAR_DICTIONARY_FILENAME, generate_rime_char_dictionary};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// 返回系统临时目录下唯一的测试输出目录路径(尚未创建)。
fn temp_output() -> PathBuf {
    std::env::temp_dir().join(format!(
        "xhup-cli-test-{}-{}",
        process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

/// 在指定输出目录运行 `generate rime`。
fn generate(output: &Path) -> Result<(), CliError> {
    let cli = Cli::try_parse_from([
        "xhup-cli",
        "generate",
        "rime",
        "--output",
        output.to_str().unwrap(),
    ])
    .unwrap();
    run(cli)
}

#[test]
fn first_generation_creates_directory_and_artifact() {
    let output = temp_output();
    let artifact = output.join(RIME_CHAR_DICTIONARY_FILENAME);
    let temporary = output.join(format!(".{RIME_CHAR_DICTIONARY_FILENAME}.tmp"));

    generate(&output).unwrap();

    assert!(artifact.is_file());
    assert_eq!(
        fs::read(&artifact).unwrap(),
        generate_rime_char_dictionary().as_bytes()
    );
    assert!(!temporary.exists(), "成功后临时文件不残留");

    fs::remove_dir_all(&output).unwrap();
}

#[test]
fn existing_artifact_is_replaced_exactly() {
    let output = temp_output();
    fs::create_dir_all(&output).unwrap();
    let artifact = output.join(RIME_CHAR_DICTIONARY_FILENAME);
    fs::write(&artifact, "垃圾内容").unwrap();

    generate(&output).unwrap();

    assert_eq!(
        fs::read(&artifact).unwrap(),
        generate_rime_char_dictionary().as_bytes()
    );

    fs::remove_dir_all(&output).unwrap();
}

#[test]
fn unrelated_files_are_preserved() {
    let output = temp_output();
    fs::create_dir_all(&output).unwrap();
    fs::write(output.join("keep.txt"), "保留我").unwrap();

    generate(&output).unwrap();

    assert_eq!(
        fs::read_to_string(output.join("keep.txt")).unwrap(),
        "保留我"
    );

    fs::remove_dir_all(&output).unwrap();
}

#[test]
fn output_path_as_regular_file_fails() {
    let output = temp_output();
    fs::write(&output, "我是一个文件").unwrap();

    let result = generate(&output);

    assert!(matches!(result, Err(CliError::OutputNotDirectory { .. })));

    fs::remove_file(&output).unwrap();
}

#[test]
fn repeated_generation_is_byte_identical() {
    let output = temp_output();
    let artifact = output.join(RIME_CHAR_DICTIONARY_FILENAME);

    generate(&output).unwrap();
    let first = fs::read(&artifact).unwrap();
    generate(&output).unwrap();
    let second = fs::read(&artifact).unwrap();

    assert_eq!(first, second);

    fs::remove_dir_all(&output).unwrap();
}
