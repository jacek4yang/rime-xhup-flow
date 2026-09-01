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
use xhup_generator::{RIME_CHAR_DICTIONARY_FILENAME, generate_rime_char_dictionary};

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
    /// 生成规范单字全码词典 xhup_flow_chars.dict.yaml
    Rime(RimeArgs),
}

#[derive(Debug, Args)]
struct RimeArgs {
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
                let artifact = generate_rime(&args.output)?;
                println!("已生成: {}", artifact.display());
                Ok(())
            }
        },
    }
}

/// 生成规范单字全码 Rime 词典到 `output` 目录,返回最终产物路径。
///
/// 先写同目录临时文件,再替换最终产物,避免正常生成过程中直接截断已有
/// 最终文件:临时文件写失败时已有最终产物不受影响;替换失败时尽力删除
/// 临时文件并返回原始替换错误。临时文件名固定,故不支持同时向同一输出
/// 目录并发生成;中断残留的临时文件会被下一次生成直接覆盖。
fn generate_rime(output: &Path) -> Result<PathBuf, CliError> {
    if output.exists() && !output.is_dir() {
        return Err(CliError::OutputNotDirectory {
            path: output.to_path_buf(),
        });
    }
    fs::create_dir_all(output).map_err(|source| CliError::CreateDirectory {
        path: output.to_path_buf(),
        source,
    })?;

    let artifact = output.join(RIME_CHAR_DICTIONARY_FILENAME);
    let temporary = output.join(format!(".{RIME_CHAR_DICTIONARY_FILENAME}.tmp"));
    let content = generate_rime_char_dictionary();
    fs::write(&temporary, content.as_bytes()).map_err(|source| CliError::WriteTemporaryFile {
        path: temporary.clone(),
        source,
    })?;
    if let Err(source) = fs::rename(&temporary, &artifact) {
        let _ = fs::remove_file(&temporary);
        return Err(CliError::ReplaceArtifact {
            temporary,
            artifact,
            source,
        });
    }
    Ok(artifact)
}
