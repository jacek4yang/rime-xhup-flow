//! `generate rime` 文件系统编排测试(不经子进程,不写仓库 dist/)。

use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};

use clap::Parser;
use xhup_cli::{Cli, CliError, run};
use xhup_generator::generate_rime_artifacts;

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

/// 校验输出目录内:每个产物字节与生成器一致,且无任何临时文件残留。
fn assert_artifacts_match_generator(output: &Path) {
    let artifacts = generate_rime_artifacts();
    assert!(!artifacts.is_empty());
    for artifact in &artifacts {
        let final_path = output.join(artifact.filename());
        assert_eq!(
            fs::read(&final_path).unwrap(),
            artifact.contents().as_bytes(),
            "{} 字节与生成器一致",
            artifact.filename()
        );
        let temporary = output.join(format!(".{}.tmp", artifact.filename()));
        assert!(
            !temporary.exists(),
            "{} 成功后临时文件不残留",
            artifact.filename()
        );
    }
}

#[test]
fn first_generation_creates_directory_and_all_artifacts() {
    let output = temp_output();

    generate(&output).unwrap();

    assert_artifacts_match_generator(&output);
    let filenames: Vec<String> = fs::read_dir(&output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        filenames.len(),
        generate_rime_artifacts().len(),
        "输出目录恰含全部产物且无其他文件"
    );

    fs::remove_dir_all(&output).unwrap();
}

#[test]
fn generated_file_set_is_exact_and_top_dictionary_imports_all_tables() {
    let output = temp_output();

    generate(&output).unwrap();

    let mut filenames: Vec<String> = fs::read_dir(&output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    filenames.sort();
    assert_eq!(
        filenames,
        [
            "xhup_flow.dict.yaml",
            "xhup_flow.schema.yaml",
            "xhup_flow_chars.dict.yaml",
            "xhup_flow_fixed_first_shortcuts.dict.yaml",
            "xhup_flow_flow.dict.yaml",
            "xhup_flow_learn.dict.yaml",
            "xhup_flow_shortcuts.dict.yaml",
            "xhup_flow_static.schema.yaml",
            "xhup_flow_two_key_shortcuts.dict.yaml",
            "xhup_flow_word_shortcuts.dict.yaml",
            "xhup_flow_words.dict.yaml",
        ],
        "输出应为且仅为 11 个 Rime 源文件"
    );
    for filename in &filenames {
        assert!(
            fs::metadata(output.join(filename)).unwrap().len() > 0,
            "{filename} 应非空"
        );
    }
    let top = fs::read_to_string(output.join("xhup_flow.dict.yaml")).unwrap();
    assert!(
        top.contains("  - xhup_flow_shortcuts"),
        "顶层词典应导入一级简码词典"
    );
    assert!(
        top.contains("  - xhup_flow_chars"),
        "顶层词典应导入单字词典"
    );
    assert!(
        top.contains("  - xhup_flow_word_shortcuts"),
        "顶层词典应导入词语简码词典"
    );
    assert!(
        top.contains("  - xhup_flow_two_key_shortcuts"),
        "顶层词典应导入二码零冲突简码词典"
    );
    assert!(
        top.contains("  - xhup_flow_words"),
        "顶层词典应导入词语词典"
    );
    assert!(
        !top.contains("xhup_flow_fixed_first_shortcuts"),
        "顶层词典不得导入 FIXED_FIRST 简码词典(由独立第二 table_translator 加载)"
    );
    assert!(
        !top.contains("xhup_flow_flow") && !top.contains("xhup_flow_learn"),
        "顶层词典不得导入 Flow 组句/学习词典(由独立 table_translator 加载)"
    );
    let schema = fs::read_to_string(output.join("xhup_flow.schema.yaml")).unwrap();
    assert!(schema.contains("xhup_flow"), "方案应引用 xhup_flow");
    assert!(
        schema.contains("table_translator@flow"),
        "主方案应含 Flow 组句 translator"
    );
    let static_schema = fs::read_to_string(output.join("xhup_flow_static.schema.yaml")).unwrap();
    assert!(
        !static_schema.contains("table_translator@flow"),
        "静态兼容方案不得含 Flow translator"
    );

    fs::remove_dir_all(&output).unwrap();
}

#[test]
fn existing_artifacts_are_replaced_exactly() {
    let output = temp_output();
    fs::create_dir_all(&output).unwrap();
    for artifact in generate_rime_artifacts() {
        fs::write(output.join(artifact.filename()), "垃圾内容").unwrap();
    }

    generate(&output).unwrap();

    assert_artifacts_match_generator(&output);

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

    generate(&output).unwrap();
    let first = read_all_artifacts(&output);
    generate(&output).unwrap();
    let second = read_all_artifacts(&output);

    assert_eq!(first, second);

    fs::remove_dir_all(&output).unwrap();
}

/// 读取输出目录内全部产物内容(按文件名排序,便于比较)。
fn read_all_artifacts(output: &Path) -> Vec<(String, Vec<u8>)> {
    let mut all: Vec<(String, Vec<u8>)> = fs::read_dir(output)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                fs::read(&path).unwrap(),
            )
        })
        .collect();
    all.sort();
    all
}
