//! XHUP Flow 命令行工具:Rime 源文件生成等开发/构建命令的编排边界。
//!
//! 本 crate 只负责参数解析与文件系统编排;生成内容由 `xhup-generator`
//! 提供,用户学习管理由 `learning` 模块(包装 librime 官方
//! `rime_dict_manager`)提供,本 crate 不重复任何业务逻辑。
#![forbid(unsafe_code)]

pub mod acceptance;
pub mod doctor;
pub mod learning;
pub mod runtime_capabilities;
pub mod user_state;

use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// 学习管理子命令参数(包装 [`learning`] 模块;本 crate 只做参数与
/// 打印编排)。
#[derive(Debug, Args)]
struct LearningArgs {
    #[command(subcommand)]
    action: LearningAction,
}

/// 本地自适应状态子命令参数(包装 [`user_state`] 模块)。
#[derive(Debug, Args)]
struct UserStateArgs {
    #[command(subcommand)]
    action: UserStateAction,
}

#[derive(Debug, Subcommand)]
enum UserStateAction {
    /// 查询本地自适应状态(快照存在性 / 规模 / 健康度;不输出词形)
    Status {
        /// Rime 用户数据目录
        #[arg(long)]
        user_data_dir: PathBuf,
    },
    /// 导出本地自适应状态快照(逐字节复制,可移植)
    Export {
        /// Rime 用户数据目录
        #[arg(long)]
        user_data_dir: PathBuf,
        /// 快照输出目录(缺省写入用户数据目录)
        #[arg(long)]
        output_dir: Option<PathBuf>,
    },
    /// 从快照恢复本地自适应状态(跨安装迁移;校验失败不触碰本地状态)
    Import {
        /// Rime 用户数据目录
        #[arg(long)]
        user_data_dir: PathBuf,
        /// 快照文件(xhup_flow_user_model.tsv)
        #[arg(long)]
        snapshot: PathBuf,
    },
    /// 重置本地自适应状态(破坏性;只删除 xhup_flow_user_model.tsv,需 --yes)
    Reset {
        /// Rime 用户数据目录
        #[arg(long)]
        user_data_dir: PathBuf,
        /// 确认破坏性重置
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Subcommand)]
enum LearningAction {
    /// 查询学习状态(用户词典 / DB / 快照存在性;不输出学习内容)
    Status {
        /// Rime 用户数据目录
        #[arg(long)]
        user_data_dir: PathBuf,
        /// rime_dict_manager 路径(缺省从 PATH 查找)
        #[arg(long)]
        dict_manager: Option<PathBuf>,
    },
    /// 导出用户词典快照(<name>.userdb.txt,标准 Rime 文本格式)
    Export {
        /// Rime 用户数据目录
        #[arg(long)]
        user_data_dir: PathBuf,
        /// 快照输出目录(缺省写入用户数据目录)
        #[arg(long)]
        output_dir: Option<PathBuf>,
        /// rime_dict_manager 路径(缺省从 PATH 查找)
        #[arg(long)]
        dict_manager: Option<PathBuf>,
    },
    /// 从快照恢复用户词典(跨安装迁移)
    Import {
        /// Rime 用户数据目录
        #[arg(long)]
        user_data_dir: PathBuf,
        /// 快照文件(文件名必须为 xhup_flow_user.userdb.txt)
        #[arg(long)]
        snapshot: PathBuf,
        /// rime_dict_manager 路径(缺省从 PATH 查找)
        #[arg(long)]
        dict_manager: Option<PathBuf>,
    },
    /// 重置用户词典(破坏性;只删除 xhup_flow_user,需 --yes)
    Reset {
        /// Rime 用户数据目录
        #[arg(long)]
        user_data_dir: PathBuf,
        /// 确认破坏性重置
        #[arg(long)]
        yes: bool,
    },
}

use clap::{Args, Parser, Subcommand};
use xhup_analyzer::user_model::UserModelLoad;
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
    /// 用户词学习管理(status / export / import / reset)
    Learning(LearningArgs),
    /// 本地自适应状态管理(status / export / import / reset;§16)
    UserState(UserStateArgs),
    /// 运行环境与 Lua 合同诊断 (doctor)
    Doctor(DoctorArgs),
    /// 校验 GA 验收清单(release/acceptance-*.json;#148)
    ValidateAcceptance(ValidateAcceptanceArgs),
    /// 对已构建的签名 RC 发布目录生成不可覆盖的 BUILD-MANIFEST.json
    SealBuild(SealBuildArgs),
    /// 输出逐平台最差验收状态(platform=STATE;供发布说明引用)
    AcceptanceSummary(AcceptanceSummaryArgs),
}

#[derive(Debug, Args)]
struct ValidateAcceptanceArgs {
    /// 验收清单 JSON 路径
    #[arg(long)]
    manifest: PathBuf,
    /// 期望版本(stable 门禁:清单 version 必须等于该值)
    #[arg(long)]
    expect_version: Option<String>,
    /// 稳定版门禁:要求完整证据及外部来源/实际工件校验
    #[arg(long, requires_all = ["expect_version", "expect_source", "artifacts_dir"])]
    stable: bool,
    /// 从 accepted RC tag 独立解析的完整源 SHA,不能从验收 JSON 复制
    #[arg(long, requires = "stable")]
    expect_source: Option<String>,
    /// 已下载的 RC 发布附件目录(包括 BUILD-MANIFEST.json)
    #[arg(long, requires = "stable")]
    artifacts_dir: Option<PathBuf>,
    /// 显式仓库所有者授权的平台待验决定；不改变默认完整验收规则
    #[arg(long, requires_all = ["stable", "qualification_proofs", "repository", "actor"])]
    runtime_qualification: Option<PathBuf>,
    /// 工作流从 GitHub 独立获取并重新汇总的证明目录
    #[arg(long, requires = "runtime_qualification")]
    qualification_proofs: Option<PathBuf>,
    #[arg(long, requires = "runtime_qualification")]
    repository: Option<String>,
    #[arg(long, requires = "runtime_qualification")]
    actor: Option<String>,
}

#[derive(Debug, Args)]
struct SealBuildArgs {
    #[arg(long)]
    version: String,
    #[arg(long)]
    source_commit: String,
    #[arg(long)]
    artifacts_dir: PathBuf,
}

#[derive(Debug, Args)]
struct AcceptanceSummaryArgs {
    /// 验收清单 JSON 路径
    #[arg(long)]
    manifest: PathBuf,
}

#[derive(Debug, Args)]
struct DoctorArgs {
    /// Rime 用户数据目录
    #[arg(long)]
    user_data_dir: PathBuf,
    /// 目标方案 (xhup_flow 或 xhup_flow_static; 缺省自适应)
    #[arg(long)]
    schema: Option<String>,
    /// 自定义 librime 插件目录 (覆盖系统与环境变量探测)
    #[arg(long)]
    plugins_dir: Option<PathBuf>,
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
    /// 生成目录包含非本包的共享 Rime 配置；生成器不是安装器，不覆盖它。
    SharedConfigurationConflict {
        /// 原始配置路径。
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
    /// 学习管理失败(status / export / import / reset)。
    Learning(learning::LearningError),
    /// 本地自适应状态管理失败。
    UserState(user_state::UserStateError),
    /// 诊断检查失败。
    Doctor(doctor::DoctorError),
    /// 验收清单无法读取。
    AcceptanceRead {
        /// 清单路径。
        path: PathBuf,
        /// 底层 I/O 错误。
        source: io::Error,
    },
    /// 验收清单不是合法 JSON 或结构不符。
    AcceptanceInvalid {
        /// 清单路径。
        path: PathBuf,
        /// serde/结构错误描述。
        source: String,
    },
    /// 验收清单存在阻塞发布项。
    AcceptanceViolations {
        /// 违例数量。
        count: usize,
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
            Self::SharedConfigurationConflict { path } => write!(
                f,
                "拒绝覆盖共享配置 {}：请生成到单独目录，再按 INSTALL.md 备份安装，或使用 Trainer",
                path.display()
            ),
            Self::WriteTemporaryFile { path, source } => {
                write!(f, "无法写入临时文件 {}: {source}", path.display())
            }
            Self::ReplaceArtifact {
                artifact, source, ..
            } => {
                write!(f, "无法替换最终产物 {}: {source}", artifact.display())
            }
            Self::Learning(source) => write!(f, "{source}"),
            Self::UserState(source) => write!(f, "{source}"),
            Self::Doctor(source) => write!(f, "{source}"),
            Self::AcceptanceInvalid { path, source } => {
                write!(f, "验收清单 {} 不合法: {source}", path.display())
            }
            Self::AcceptanceRead { path, source } => {
                write!(f, "无法读取验收清单 {}: {source}", path.display())
            }
            Self::AcceptanceViolations { count } => write!(
                f,
                "验收清单存在 {count} 项阻塞发布(逐项见上方 ::error:: 行)"
            ),
        }
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CreateDirectory { source, .. }
            | Self::WriteTemporaryFile { source, .. }
            | Self::ReplaceArtifact { source, .. }
            | Self::AcceptanceRead { source, .. } => Some(source),
            Self::Learning(source) => Some(source),
            Self::UserState(source) => Some(source),
            Self::Doctor(source) => Some(source),
            Self::OutputNotDirectory { .. }
            | Self::SharedConfigurationConflict { .. }
            | Self::AcceptanceInvalid { .. }
            | Self::AcceptanceViolations { .. } => None,
        }
    }
}

impl From<learning::LearningError> for CliError {
    fn from(source: learning::LearningError) -> Self {
        Self::Learning(source)
    }
}

impl From<user_state::UserStateError> for CliError {
    fn from(source: user_state::UserStateError) -> Self {
        Self::UserState(source)
    }
}

impl From<doctor::DoctorError> for CliError {
    fn from(source: doctor::DoctorError) -> Self {
        Self::Doctor(source)
    }
}

/// 执行解析后的命令。
///
/// 成功时向 stdout 打印一行结果;失败返回携带路径上下文的 [`CliError`]。
pub fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Generate(args) => match args.target {
            GenerateTarget::Rime(args) => {
                use std::io::Read as _;
                let path = args.output.join("default.custom.yaml");
                let expected = include_bytes!("../../../rime/package/default.custom.yaml");
                let conflict = || CliError::SharedConfigurationConflict { path: path.clone() };
                match fs::symlink_metadata(&path) {
                    Ok(metadata) => {
                        if !metadata.is_file()
                            || metadata.file_type().is_symlink()
                            || metadata.len() != expected.len() as u64
                        {
                            return Err(conflict());
                        }
                        let mut bytes = Vec::new();
                        fs::File::open(&path)
                            .map_err(|_| conflict())?
                            .take(expected.len() as u64 + 1)
                            .read_to_end(&mut bytes)
                            .map_err(|_| conflict())?;
                        if bytes != expected {
                            return Err(conflict());
                        }
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                        ) => {}
                    Err(_) => return Err(conflict()),
                }
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
        Command::Learning(args) => match args.action {
            LearningAction::Status {
                user_data_dir,
                dict_manager,
            } => {
                let status = learning::status(&user_data_dir, dict_manager.as_deref())?;
                println!("用户词典: {}", status.user_dict);
                println!("用户数据目录: {}", status.user_data_dir.display());
                if status.db_exists {
                    println!(
                        "用户词典 DB: 存在({})",
                        status.db_path.expect("db_exists 时必有路径").display()
                    );
                } else {
                    println!("用户词典 DB: 不存在(尚无学习数据)");
                }
                if let Some(snapshot) = status.snapshot_path {
                    println!("已有快照: {}", snapshot.display());
                }
                if !status.known_user_dicts.is_empty() {
                    println!("本目录用户词典: {}", status.known_user_dicts.join(", "));
                }
                Ok(())
            }
            LearningAction::Export {
                user_data_dir,
                output_dir,
                dict_manager,
            } => {
                let snapshot = learning::export(
                    &user_data_dir,
                    output_dir.as_deref(),
                    dict_manager.as_deref(),
                )?;
                println!("已导出快照: {}", snapshot.display());
                Ok(())
            }
            LearningAction::Import {
                user_data_dir,
                snapshot,
                dict_manager,
            } => {
                learning::import(&user_data_dir, &snapshot, dict_manager.as_deref())?;
                println!("已从快照恢复: {}", snapshot.display());
                Ok(())
            }
            LearningAction::Reset { user_data_dir, yes } => {
                learning::reset(&user_data_dir, yes)?;
                println!("已重置用户词典 {}", learning::FLOW_USER_DICT_NAME);
                Ok(())
            }
        },
        Command::UserState(args) => match args.action {
            UserStateAction::Status { user_data_dir } => {
                let file = user_state::path(&user_data_dir);
                let (model, status) = user_state::load(&user_data_dir)?;
                println!("状态快照: {}", file.display());
                match status {
                    UserModelLoad::Ok if model.is_empty() => {
                        println!("状态: 空(尚无本地自适应数据或已重置)");
                    }
                    UserModelLoad::Ok => {
                        println!("状态: 正常(已学习 {} 个词)", model.len());
                    }
                    reason => {
                        println!("状态: 降级 —— {reason}");
                    }
                }
                Ok(())
            }
            UserStateAction::Export {
                user_data_dir,
                output_dir,
            } => {
                let target = user_state::export(&user_data_dir, output_dir.as_deref())?;
                println!("已导出快照: {}", target.display());
                Ok(())
            }
            UserStateAction::Import {
                user_data_dir,
                snapshot,
            } => {
                user_state::import(&user_data_dir, &snapshot)?;
                println!("已从快照恢复: {}", snapshot.display());
                Ok(())
            }
            UserStateAction::Reset { user_data_dir, yes } => {
                user_state::reset(&user_data_dir, yes)?;
                println!("已重置本地自适应状态 {}", user_state::USER_MODEL_FILENAME);
                Ok(())
            }
        },
        Command::SealBuild(args) => {
            acceptance::provenance::seal(&args.version, &args.source_commit, &args.artifacts_dir)
                .map_err(|source| CliError::AcceptanceInvalid {
                path: args.artifacts_dir,
                source: source.to_string(),
            })?;
            println!("BUILD-MANIFEST.json 已生成(仅记录构建,不代表验收 PASS)");
            Ok(())
        }
        Command::ValidateAcceptance(args) => {
            let json =
                fs::read_to_string(&args.manifest).map_err(|source| CliError::AcceptanceRead {
                    path: args.manifest.clone(),
                    source,
                })?;
            let manifest = acceptance::parse_manifest(&json).map_err(|source| {
                CliError::AcceptanceInvalid {
                    path: args.manifest.clone(),
                    source: source.to_string(),
                }
            })?;
            let violations = if let Some(path) = &args.runtime_qualification {
                let bytes = fs::read(path).map_err(|source| CliError::AcceptanceRead {
                    path: path.clone(),
                    source,
                })?;
                let authorization = serde_json::from_slice(&bytes).map_err(|source| {
                    CliError::AcceptanceInvalid {
                        path: path.clone(),
                        source: source.to_string(),
                    }
                })?;
                let context = acceptance::runtime_qualification::Context {
                    version: args
                        .expect_version
                        .as_deref()
                        .expect("clap requires expect-version"),
                    source: args
                        .expect_source
                        .as_deref()
                        .expect("clap requires expect-source"),
                    artifacts: args
                        .artifacts_dir
                        .as_deref()
                        .expect("clap requires artifacts-dir"),
                    proofs: args
                        .qualification_proofs
                        .as_deref()
                        .expect("clap requires qualification-proofs"),
                    repository: args
                        .repository
                        .as_deref()
                        .expect("clap requires repository"),
                    actor: args.actor.as_deref().expect("clap requires actor"),
                };
                acceptance::runtime_qualification::verify(&manifest, &authorization, &context)
            } else if args.stable {
                acceptance::provenance::verify(
                    &manifest,
                    args.expect_version
                        .as_deref()
                        .expect("clap requires expect-version"),
                    args.expect_source
                        .as_deref()
                        .expect("clap requires expect-source"),
                    args.artifacts_dir
                        .as_deref()
                        .expect("clap requires artifacts-dir"),
                )
            } else {
                acceptance::check_rc(&manifest)
            };
            if violations.is_empty() {
                let mode = if args.runtime_qualification.is_some() {
                    acceptance::runtime_qualification::POLICY
                } else if args.stable {
                    "stable"
                } else {
                    "rc"
                };
                println!(
                    "验收清单校验通过({mode} 门禁): {} v{}",
                    args.manifest.display(),
                    manifest.version
                );
                Ok(())
            } else {
                for violation in &violations {
                    eprintln!("::error::{violation}");
                }
                Err(CliError::AcceptanceViolations {
                    count: violations.len(),
                })
            }
        }
        Command::AcceptanceSummary(args) => {
            let json =
                fs::read_to_string(&args.manifest).map_err(|source| CliError::AcceptanceRead {
                    path: args.manifest.clone(),
                    source,
                })?;
            let manifest = acceptance::parse_manifest(&json).map_err(|source| {
                CliError::AcceptanceInvalid {
                    path: args.manifest.clone(),
                    source: source.to_string(),
                }
            })?;
            for line in acceptance::per_platform_summary(&manifest) {
                println!("{line}");
            }
            Ok(())
        }
        Command::Doctor(args) => {
            let report = doctor::inspect_installation(
                &args.user_data_dir,
                args.schema.as_deref(),
                args.plugins_dir.as_deref(),
            )?;
            print!("{}", report.format_report());
            if !report.lua_contract_ok {
                return Err(doctor::DoctorError::ContractFailed(
                    "Lua 运行时合同未满足，详情见上方报告".to_string(),
                )
                .into());
            }
            Ok(())
        }
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
        // 产物可含子目录(如 lua/xhup_flow/…):父目录必须先创建;
        // 临时文件与最终产物同目录,保证 rename 同卷原子。
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent).map_err(|source| CliError::CreateDirectory {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let temporary = final_path.with_file_name(format!(
            ".{}.tmp",
            final_path
                .file_name()
                .expect("产物文件名应有基名")
                .to_string_lossy()
        ));
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
