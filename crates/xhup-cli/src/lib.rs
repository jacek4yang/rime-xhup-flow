//! XHUP Flow 命令行工具:Rime 源文件生成等开发/构建命令的编排边界。
//!
//! 本 crate 只负责参数解析与文件系统编排;生成内容由 `xhup-generator`
//! 提供,本 crate 不重复任何生成逻辑。
#![forbid(unsafe_code)]

use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand};
use xhup_generator::{TRAINER_DATA_FILENAME, generate_rime_artifacts, generate_trainer_dataset};

/// XHUP Flow 命令行工具的参数模型(经 clap 解析构造)。
#[derive(Debug, Parser)]
#[command(name = "xhup-cli", about = "XHUP Flow 开发/构建命令行工具")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// 生成当前已支持的 Rime 源文件
    Generate(GenerateArgs),
}

#[derive(Debug, Args)]
struct GenerateArgs {
    #[command(subcommand)]
    target: GenerateTarget,
}

#[derive(Debug, Subcommand)]
enum GenerateTarget {
    /// 生成便携 Rime 源包(单字词典、顶层词典与方案)
    Rime(OutputArgs),
    /// 生成训练器规范数据集 xhup_flow_trainer.json
    Trainer(OutputArgs),
}

#[derive(Debug, Args)]
struct OutputArgs {
    /// 生成产物输出目录(不存在则递归创建)
    #[arg(long)]
    output: PathBuf,
}

/// 命令执行错误。
#[derive(Debug)]
pub enum CliError {
    /// 无法创建输出目录。
    CreateDirectory {
        /// 输出目录路径。
        path: PathBuf,
        /// 底层 I/O 错误。
        source: io::Error,
    },
    /// 输出路径已存在但不是目录。
    OutputNotDirectory {
        /// 输出路径。
        path: PathBuf,
    },
    /// 无法写入临时产物文件。
    WriteTemporaryFile {
        /// 临时文件路径。
        path: PathBuf,
        /// 底层 I/O 错误。
        source: io::Error,
    },
    /// 无法用临时产物替换最终产物。
    ReplaceArtifact {
        /// 临时文件路径。
        temporary: PathBuf,
        /// 最终产物路径。
        artifact: PathBuf,
        /// 底层 I/O 错误。
        source: io::Error,
    },
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateDirectory { path, source } => {
                write!(f, "无法创建输出目录 {}: {source}", path.display())
            }
            Self::OutputNotDirectory { path } => {
                write!(f, "输出路径不是目录: {}", path.display())
            }
            Self::WriteTemporaryFile { path, source } => {
                write!(f, "无法写入临时文件 {}: {source}", path.display())
            }
            Self::ReplaceArtifact {
                artifact, source, ..
            } => {
                write!(f, "无法替换最终产物 {}: {source}", artifact.display())
            }
        }
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CreateDirectory { source, .. }
            | Self::WriteTemporaryFile { source, .. }
            | Self::ReplaceArtifact { source, .. } => Some(source),
            Self::OutputNotDirectory { .. } => None,
        }
    }
}

/// 执行解析后的命令。
///
/// 成功时向 stdout 打印一行结果;失败返回携带路径上下文的 [`CliError`]。
pub fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Generate(args) => match args.target {
            GenerateTarget::Rime(args) => {
                let artifacts = generate_rime_artifacts();
                let files: Vec<(&str, &str)> = artifacts
                    .iter()
                    .map(|artifact| (artifact.filename(), artifact.contents()))
                    .collect();
                let count = write_outputs(&args.output, &files)?;
                println!("已生成 {count} 个 Rime 源文件: {}", args.output.display());
                Ok(())
            }
            GenerateTarget::Trainer(args) => {
                let dataset = generate_trainer_dataset();
                write_outputs(&args.output, &[(TRAINER_DATA_FILENAME, dataset.as_str())])?;
                println!(
                    "已生成训练器数据集: {}",
                    args.output.join(TRAINER_DATA_FILENAME).display()
                );
                Ok(())
            }
        },
    }
}

/// 把 `(文件名, 内容)` 集合安全写入 `output` 目录,返回产物数量。
///
/// 先在内存备好全部产物内容,再把每个产物的同目录临时文件全部写完,
/// 最后逐个替换最终产物:临时文件写失败时尚未替换任何最终产物,尽力
/// 清理已写临时文件后返回原始错误;替换阶段不是事务,中途失败可能留下
/// 部分更新的包(已接受的限制),此时尽力清理剩余临时文件并返回原始
/// 替换错误。临时文件名固定,故不支持同时向同一输出目录并发生成;
/// 中断残留的临时文件会被下一次生成直接覆盖。
fn write_outputs(output: &Path, files: &[(&str, &str)]) -> Result<usize, CliError> {
    if output.exists() && !output.is_dir() {
        return Err(CliError::OutputNotDirectory {
            path: output.to_path_buf(),
        });
    }
    fs::create_dir_all(output).map_err(|source| CliError::CreateDirectory {
        path: output.to_path_buf(),
        source,
    })?;

    let mut prepared = Vec::with_capacity(files.len());
    for (filename, contents) in files {
        let final_path = output.join(filename);
        let temporary = output.join(format!(".{filename}.tmp"));
        if let Err(source) = fs::write(&temporary, contents.as_bytes()) {
            for (temporary, _) in &prepared {
                let _ = fs::remove_file(temporary);
            }
            let _ = fs::remove_file(&temporary);
            return Err(CliError::WriteTemporaryFile {
                path: temporary,
                source,
            });
        }
        prepared.push((temporary, final_path));
    }

    let count = prepared.len();
    for (index, (temporary, final_path)) in prepared.iter().enumerate() {
        if let Err(source) = fs::rename(temporary, final_path) {
            for (temporary, _) in &prepared[index..] {
                let _ = fs::remove_file(temporary);
            }
            return Err(CliError::ReplaceArtifact {
                temporary: temporary.clone(),
                artifact: final_path.clone(),
                source,
            });
        }
    }
    Ok(count)
}
