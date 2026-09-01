//! `generate trainer` 文件系统编排测试(不经子进程,不写仓库 dist/)。

use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};

use clap::Parser;
use xhup_cli::{Cli, CliError, run};
use xhup_generator::{TRAINER_DATA_FILENAME, generate_trainer_dataset};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// 返回系统临时目录下唯一的测试输出目录路径(尚未创建)。
fn temp_output() -> PathBuf {
    std::env::temp_dir().join(format!(
        "xhup-cli-trainer-test-{}-{}",
        process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

/// 在指定输出目录运行 `generate trainer`。
fn generate(output: &Path) -> Result<(), CliError> {
    let cli = Cli::try_parse_from([
        "xhup-cli",
        "generate",
        "trainer",
        "--output",
        output.to_str().unwrap(),
    ])
    .unwrap();
    run(cli)
}

fn dataset_path(output: &Path) -> PathBuf {
    output.join(TRAINER_DATA_FILENAME)
}

/// 校验输出目录:数据集字节与生成器一致,且无临时文件残留。
fn assert_dataset_matches_generator(output: &Path) {
    assert_eq!(
        fs::read(dataset_path(output)).unwrap(),
        generate_trainer_dataset().as_bytes(),
        "数据集字节与生成器一致"
    );
    let temporary = output.join(format!(".{TRAINER_DATA_FILENAME}.tmp"));
    assert!(!temporary.exists(), "成功后临时文件不残留");
}

#[test]
fn first_generation_creates_directory_and_dataset() {
    let output = temp_output();

    generate(&output).unwrap();

    assert_dataset_matches_generator(&output);
    let filenames: Vec<String> = fs::read_dir(&output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(filenames, [TRAINER_DATA_FILENAME], "输出目录恰含数据集");

    fs::remove_dir_all(&output).unwrap();
}

#[test]
fn existing_dataset_is_replaced_exactly() {
    let output = temp_output();
    fs::create_dir_all(&output).unwrap();
    fs::write(dataset_path(&output), "垃圾内容").unwrap();

    generate(&output).unwrap();

    assert_dataset_matches_generator(&output);

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
fn output_option_is_required() {
    let result = Cli::try_parse_from(["xhup-cli", "generate", "trainer"]);
    assert!(result.is_err(), "缺少 --output 应解析失败");
}

#[test]
fn repeated_generation_is_byte_identical() {
    let output = temp_output();

    generate(&output).unwrap();
    let first = fs::read(dataset_path(&output)).unwrap();
    generate(&output).unwrap();
    let second = fs::read(dataset_path(&output)).unwrap();

    assert_eq!(first, second);

    fs::remove_dir_all(&output).unwrap();
}
