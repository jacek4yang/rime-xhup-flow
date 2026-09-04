//! XHUP Flow 产品管理核心:Rime 安装 / 升级 / 修复 / 卸载 / 诊断。
//!
//! 设计约束(见里程碑 D):
//! - **纯逻辑层**:不依赖 Tauri 类型与真实平台 API(目录发现仅用
//!   标准库 + 环境变量),可被 Tauri 命令与单元测试共同复用;
//! - **所有权清单**:安装只写 XHUP 拥有的文件;卸载只删拥有文件;
//!   绝不触碰用户其它 Rime 配置与学习数据(`xhup_flow_user.userdb`);
//! - **计划先于动作**:install/update/repair 都先产出 [`Plan`](行动
//!   清单),由调用方确认后执行;支持 dry-run 测试;
//! - **覆盖前备份**:升级/修复会先备份将被覆盖的 XHUP 文件;
//! - **原子写入**:文件内容先写同目录临时文件,再原子替换最终产物;
//! - **本地优先**:无网络、无遥测;学习管理复用 `xhup-cli` 的
//!   `learning` 模块(包装 librime 官方 `rime_dict_manager`),
//!   不在此实现第二份 userdb 管理。

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// XHUP Flow 拥有的 Rime 源文件清单(与 `generate_rime_artifacts` 的
/// 产物一致;安装即写这些文件)。所有权唯一来源:改生成器产物集合时
/// 必须同步本清单,`bundled_package_matches_manifest` 测试兜底。
pub const OWNED_FILES: &[&str] = &[
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
];

/// 主方案 / 静态回退方案的 schema id(模式选择只在这两者之间)。
pub const FLOW_SCHEMA_ID: &str = "xhup_flow";
pub const STATIC_SCHEMA_ID: &str = "xhup_flow_static";

/// 平台上检测到的 Rime 客户端(Android 客户端不做桌面端自动安装)。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum RimeClient {
    /// Windows / 小狼毫。
    Weasel,
    /// macOS / 鼠须管。
    Squirrel,
    /// Linux / Fcitx5-Rime。
    Fcitx5,
    /// Linux / IBus-Rime。
    Ibus,
}

impl RimeClient {
    /// 该客户端默认的用户数据目录(存在性由调用方检查)。
    ///
    /// 路径来源均为各客户端源码/官方文档(fcitx5-rime 为
    /// `$XDG_DATA_HOME/fcitx5/rime`,即默认 `~/.local/share/fcitx5/rime`;
    /// `.config` 是 fcitx4 时代约定,作回退)。
    pub fn default_user_data_dir(&self) -> Option<PathBuf> {
        match self {
            Self::Weasel => {
                std::env::var_os("APPDATA").map(|base| PathBuf::from(base).join("Rime"))
            }
            Self::Squirrel => {
                std::env::var_os("HOME").map(|base| PathBuf::from(base).join("Library/Rime"))
            }
            Self::Fcitx5 => std::env::var_os("XDG_DATA_HOME")
                .map(|base| PathBuf::from(base).join("fcitx5/rime"))
                .or_else(|| {
                    std::env::var_os("HOME")
                        .map(|base| PathBuf::from(base).join(".local/share/fcitx5/rime"))
                })
                .or_else(|| {
                    // fcitx4 时代遗留路径回退。
                    std::env::var_os("XDG_CONFIG_HOME")
                        .map(|base| PathBuf::from(base).join("fcitx5/rime"))
                        .or_else(|| {
                            std::env::var_os("HOME")
                                .map(|base| PathBuf::from(base).join(".config/fcitx5/rime"))
                        })
                }),
            Self::Ibus => std::env::var_os("XDG_DATA_HOME")
                .map(|base| PathBuf::from(base).join("ibus/rime"))
                .or_else(|| {
                    std::env::var_os("HOME")
                        .map(|base| PathBuf::from(base).join(".config/ibus/rime"))
                }),
        }
    }

    /// 该客户端自动部署机制的官方参数(与程序无关,纯常量,便于测试)。
    /// 来源为各客户端源码(见 `redeploy_candidates` 文档)。
    pub fn redeploy_arguments(&self) -> &'static [&'static str] {
        match self {
            Self::Weasel => &["/deploy"],
            Self::Squirrel => &["--reload"],
            Self::Fcitx5 => &[
                "--session",
                "--print-reply",
                "--dest=org.fcitx.Fcitx5",
                "/controller",
                "org.fcitx.Fcitx.Controller1.SetConfig",
                "string:fcitx://config/addon/rime/deploy",
                "variant:string:",
            ],
            Self::Ibus => &["restart"],
        }
    }

    /// 官方重新部署机制的候选(按优先级;来源为各客户端源码,存在性
    /// 由调用方探测):
    ///
    /// - Weasel:`WeaselDeployer.exe /deploy`(与官方托盘菜单同一机制,
    ///   参数必须恰好为 `/deploy`);
    /// - Squirrel:`Squirrel --reload`(向运行中的进程投递部署通知,
    ///   进程内完整部署,不退出应用);
    /// - Fcitx5:经 `dbus-send` 调用 Fcitx5 官方 D-Bus 接口
    ///   `SetConfig fcitx://config/addon/rime/deploy`(进程内完整部署);
    /// - Ibus:`ibus restart`(官方 CLI,重启 ibus-daemon,所有引擎
    ///   重新部署;不杀任意进程)。
    pub fn redeploy_candidates(&self) -> Vec<(PathBuf, Vec<String>)> {
        let args: Vec<String> = self
            .redeploy_arguments()
            .iter()
            .map(|s| s.to_string())
            .collect();
        match self {
            Self::Weasel => {
                let mut candidates = Vec::new();
                for base in ["ProgramFiles", "ProgramFiles(x86)"] {
                    if let Some(dir) = std::env::var_os(base) {
                        candidates.push((
                            PathBuf::from(dir).join("Rime").join("WeaselDeployer.exe"),
                            args.clone(),
                        ));
                    }
                }
                candidates
            }
            Self::Squirrel => vec![(
                PathBuf::from("/Library/Input Methods/Squirrel.app/Contents/MacOS/Squirrel"),
                args,
            )],
            Self::Fcitx5 => find_in_path("dbus-send")
                .map(|program| vec![(program, args)])
                .unwrap_or_default(),
            Self::Ibus => find_in_path("ibus")
                .map(|program| vec![(program, args)])
                .unwrap_or_default(),
        }
    }

    /// 能力声明:探测候选可执行文件,返回 [`RedeploySupport`]。
    ///
    /// 生产探测 = 文件存在(`Path::is_file`);测试可用
    /// [`resolve_redeploy`] 注入假候选。
    pub fn redeploy_support(&self) -> RedeploySupport {
        resolve_redeploy(&self.redeploy_candidates(), &|path| path.is_file())
    }

    /// 手动重新部署的操作指引(无自动机制时的兜底;不发明命令)。
    pub fn redeploy_guidance(&self) -> &'static str {
        match self {
            Self::Weasel => "在系统托盘的「小狼毫」菜单中执行「重新部署」。",
            Self::Squirrel => "在菜单栏「鼠须管」菜单中执行「重新部署」。",
            Self::Fcitx5 => "运行 fcitx5-rime 的「重新部署」,或重启 Fcitx5。",
            Self::Ibus => "运行 ibus restart,或在 IBus 设置中重新部署 Rime。",
        }
    }
}

/// 重新部署能力(能力型 UI 依据;自动执行仅使用上面列出的官方机制)。
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum RedeploySupport {
    /// 检测到官方部署机制,可自动执行(结构化参数,无 shell)。
    Automatic {
        program: PathBuf,
        args: Vec<std::ffi::OsString>,
    },
    /// 无可靠自动机制,按 [`RimeClient::redeploy_guidance`] 手动执行。
    Manual,
}

/// 在候选列表中探测第一个存在的可执行文件(纯函数,测试可注入)。
pub fn resolve_redeploy(
    candidates: &[(PathBuf, Vec<String>)],
    probe: &dyn Fn(&Path) -> bool,
) -> RedeploySupport {
    for (program, args) in candidates {
        if probe(program) {
            return RedeploySupport::Automatic {
                program: program.clone(),
                args: args.iter().map(std::ffi::OsString::from).collect(),
            };
        }
    }
    RedeploySupport::Manual
}

/// 在 PATH 中查找可执行文件(仅在 PATH 目录内,不扫描任意磁盘)。
fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// 管理错误。
#[derive(Debug)]
pub enum ManagerError {
    /// 用户数据目录不存在(未安装 Rime 或目录不合法)。
    UserDataDirMissing { path: PathBuf },
    /// 源包不合法(缺少 schema 或词典文件)。
    PackageInvalid { missing: String },
    /// 计划执行时需要源包但未提供。
    PackageMissing,
    /// 计划执行时源包缺少文件。
    SourceMissing { file: String },
    /// 文件系统操作失败。
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for ManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserDataDirMissing { path } => {
                write!(f, "Rime 用户数据目录不存在:{}", path.display())
            }
            Self::PackageInvalid { missing } => write!(f, "源包不合法(缺少 {missing})"),
            Self::PackageMissing => write!(f, "执行安装计划需要源包"),
            Self::SourceMissing { file } => write!(f, "源包缺少文件:{file}"),
            Self::Io { path, source } => write!(f, "文件操作失败 {}: {source}", path.display()),
        }
    }
}

impl ManagerError {
    /// 稳定的机器可读错误码(跨前端/后端契约,不做字符串匹配)。
    pub fn code(&self) -> &'static str {
        match self {
            Self::UserDataDirMissing { .. } => "user_data_dir_missing",
            Self::PackageInvalid { .. } => "package_invalid",
            Self::PackageMissing => "package_missing",
            Self::SourceMissing { .. } => "source_missing",
            Self::Io { .. } => "io",
        }
    }
}

impl std::error::Error for ManagerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// 内存中的 Rime 源包:版本 + 全部生成产物。
///
/// 调用方通过 [`RimePackage::bundled`] 获得当前生成器产物;测试可直接
/// 构造假包。不落盘、不涉及真实文件系统。
#[derive(Clone, Debug)]
pub struct RimePackage {
    /// 包版本(取自主方案文件头 `version:`)。
    pub version: String,
    /// `(文件名, 内容)` 集合,覆盖全部 [`OWNED_FILES`]。
    pub files: Vec<(String, String)>,
}

impl RimePackage {
    /// 从当前生成器产物构建源包(`xhup-generator` 是唯一语义来源)。
    ///
    /// 生成器产物缺少主方案或其内嵌版本属源码级缺陷:返回
    /// [`ManagerError::PackageInvalid`] 而不是 panic(命令路径不得崩溃)。
    pub fn bundled() -> Result<Self, ManagerError> {
        let files: Vec<(String, String)> = xhup_generator::generate_rime_artifacts()
            .into_iter()
            .map(|artifact| {
                (
                    artifact.filename().to_string(),
                    artifact.contents().to_string(),
                )
            })
            .collect();
        let schema_file = format!("{FLOW_SCHEMA_ID}.schema.yaml");
        let version = files
            .iter()
            .find(|(name, _)| name == &schema_file)
            .and_then(|(_, contents)| parse_schema_version(contents))
            .ok_or_else(|| ManagerError::PackageInvalid {
                missing: format!("{schema_file}#version"),
            })?;
        Ok(Self { version, files })
    }

    /// 按文件名取产物内容。
    pub fn contents_of(&self, file: &str) -> Option<&str> {
        self.files
            .iter()
            .find(|(name, _)| name == file)
            .map(|(_, contents)| contents.as_str())
    }
}

/// 从 Rime 方案 YAML 文件头解析 `version:` 值(去引号;缺失返回 `None`)。
pub fn parse_schema_version(contents: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let trimmed = line.trim();
        let value = trimmed.strip_prefix("version:")?.trim();
        let value = value.trim_matches('"').trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

/// 平台探测:按当前编译平台返回候选客户端与默认用户数据目录。
pub fn detect_platform() -> (RimeClient, Option<PathBuf>) {
    #[cfg(target_os = "windows")]
    {
        let client = RimeClient::Weasel;
        (client, client.default_user_data_dir())
    }
    #[cfg(target_os = "macos")]
    {
        let client = RimeClient::Squirrel;
        (client, client.default_user_data_dir())
    }
    #[cfg(target_os = "linux")]
    {
        // Linux 上按用户数据目录哪个存在决定客户端;都不存在时默认 fcitx5。
        let fcitx5 = RimeClient::Fcitx5.default_user_data_dir();
        let ibus = RimeClient::Ibus.default_user_data_dir();
        if fcitx5.as_deref().is_some_and(Path::is_dir) {
            (RimeClient::Fcitx5, fcitx5)
        } else if ibus.as_deref().is_some_and(Path::is_dir) {
            (RimeClient::Ibus, ibus)
        } else {
            (RimeClient::Fcitx5, fcitx5)
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        (RimeClient::Fcitx5, None)
    }
}

/// 一条计划动作。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlanAction {
    /// 新建文件(安装 / 修复)。
    Write { file: String },
    /// 覆盖既有 XHUP 文件(升级;执行前先备份)。
    Overwrite { file: String, backup: PathBuf },
    /// 删除 XHUP 文件(卸载)。
    Delete { file: String },
}

impl PlanAction {
    /// 动作涉及的拥有文件名。
    pub fn file(&self) -> &str {
        match self {
            Self::Write { file } | Self::Overwrite { file, .. } | Self::Delete { file } => file,
        }
    }
}

/// 行动计划(dry-run 的产物;调用方确认后交给 [`execute`])。
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub actions: Vec<PlanAction>,
    /// 计划外的说明(如重新部署指引)。
    pub notes: Vec<String>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

/// 单个拥有文件的安装完整性(与随附源包逐字节比对;不标记用户自有
/// Rime 文件)。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileIntegrity {
    /// 拥有清单中的文件不存在。
    Missing,
    /// 存在且与随附包字节一致。
    Match,
    /// 存在但与随附包不同(旧版本或被外部改动)。
    Different,
}

/// 安装健康分类(由调用方结合随附包版本推导;见
/// `commands::product_status`)。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallHealth {
    NotInstalled,
    Healthy,
    UpdateAvailable,
    Modified,
    Incomplete,
}

impl InstallStatus {
    /// 由安装状态推导健康分类(需要随附包版本比较)。
    ///
    /// `Modified` 仅指「已安装版本与随附版本一致但内容被外部改动」;
    /// 旧版本文件与随附包天然不同,归入 `UpdateAvailable`。
    pub fn health(&self, bundled_version: &str) -> InstallHealth {
        if self.installed_files == 0 {
            return InstallHealth::NotInstalled;
        }
        if !self.missing_files.is_empty() {
            return InstallHealth::Incomplete;
        }
        let same_version = self.installed_version.as_deref() == Some(bundled_version);
        let has_difference = self.integrity.contains(&FileIntegrity::Different);
        if same_version && has_difference {
            return InstallHealth::Modified;
        }
        if !same_version {
            return InstallHealth::UpdateAvailable;
        }
        InstallHealth::Healthy
    }
}

/// 用户数据目录内的 XHUP 安装状态。
#[derive(Clone, Debug, Serialize)]
pub struct InstallStatus {
    pub user_data_dir: PathBuf,
    pub client: RimeClient,
    /// 已安装的 XHUP 文件数(0 = 未安装)。
    pub installed_files: usize,
    /// 拥有清单文件总数(前端展示进度用)。
    pub total_files: usize,
    /// 拥有清单中缺失的文件(已安装但损坏时 > 0)。
    pub missing_files: Vec<String>,
    /// 已安装的 XHUP 方案 id(Flow / Static / 两者)。
    pub schemas: Vec<String>,
    /// 已安装包的版本(解析自已安装的主方案文件头;未安装时 `None`)。
    pub installed_version: Option<String>,
    /// 每个拥有文件相对随附包的完整性(与 OWNED_FILES 同序)。
    pub integrity: Vec<FileIntegrity>,
}

/// 检查用户数据目录内的安装状态。
///
/// `package` 提供时逐文件做完整性比对(字节相等);`None` 时所有
/// 已存在文件记为 Match(不比对,仅存在性)。
pub fn install_status(
    user_data_dir: &Path,
    client: RimeClient,
    package: Option<&RimePackage>,
) -> InstallStatus {
    let mut installed_files = 0;
    let mut missing_files = Vec::new();
    let mut integrity = Vec::new();
    for file in OWNED_FILES {
        let target = user_data_dir.join(file);
        if target.is_file() {
            installed_files += 1;
            let state = match package.and_then(|package| package.contents_of(file)) {
                Some(expected) => match fs::read(&target) {
                    Ok(actual) if actual.as_slice() == expected.as_bytes() => FileIntegrity::Match,
                    Ok(_) | Err(_) => FileIntegrity::Different,
                },
                None => FileIntegrity::Match,
            };
            integrity.push(state);
        } else {
            missing_files.push((*file).to_string());
            integrity.push(FileIntegrity::Missing);
        }
    }
    let mut schemas = Vec::new();
    let mut installed_version = None;
    let schema_file = format!("{FLOW_SCHEMA_ID}.schema.yaml");
    if user_data_dir.join(&schema_file).is_file() {
        schemas.push(FLOW_SCHEMA_ID.to_string());
        installed_version = fs::read_to_string(user_data_dir.join(&schema_file))
            .ok()
            .as_deref()
            .and_then(parse_schema_version);
    }
    let static_schema_file = format!("{STATIC_SCHEMA_ID}.schema.yaml");
    if user_data_dir.join(&static_schema_file).is_file() {
        schemas.push(STATIC_SCHEMA_ID.to_string());
    }
    InstallStatus {
        user_data_dir: user_data_dir.to_path_buf(),
        client,
        installed_files,
        total_files: OWNED_FILES.len(),
        missing_files,
        schemas,
        installed_version,
        integrity,
    }
}

/// 校验源包:必须包含全部拥有文件(否则拒绝安装)。
fn validate_package(package: &RimePackage) -> Result<(), ManagerError> {
    for file in OWNED_FILES {
        if package.contents_of(file).is_none() {
            return Err(ManagerError::PackageInvalid {
                missing: (*file).to_string(),
            });
        }
    }
    Ok(())
}

/// 产出安装 / 升级 / 修复计划。
///
/// - 未安装 → 全部 Write;
/// - 已安装 → 已有 XHUP 文件 Overwrite(带备份),缺失文件 Write(修复);
/// - 源包必须通过校验;`xhup_flow_user.userdb` 永远不在计划内。
pub fn plan_install(user_data_dir: &Path, package: &RimePackage) -> Result<Plan, ManagerError> {
    if !user_data_dir.is_dir() {
        return Err(ManagerError::UserDataDirMissing {
            path: user_data_dir.to_path_buf(),
        });
    }
    validate_package(package)?;
    let mut actions = Vec::new();
    for file in OWNED_FILES {
        let target = user_data_dir.join(file);
        if target.exists() {
            actions.push(PlanAction::Overwrite {
                file: (*file).to_string(),
                backup: backup_path(user_data_dir, file),
            });
        } else {
            actions.push(PlanAction::Write {
                file: (*file).to_string(),
            });
        }
    }
    Ok(Plan {
        actions,
        notes: vec![],
    })
}

/// 产出卸载计划:只删拥有文件(用户数据目录必须存在)。
pub fn plan_uninstall(user_data_dir: &Path) -> Result<Plan, ManagerError> {
    if !user_data_dir.is_dir() {
        return Err(ManagerError::UserDataDirMissing {
            path: user_data_dir.to_path_buf(),
        });
    }
    let actions = OWNED_FILES
        .iter()
        .filter(|file| user_data_dir.join(file).is_file())
        .map(|file| PlanAction::Delete {
            file: (*file).to_string(),
        })
        .collect();
    Ok(Plan {
        actions,
        notes: vec![],
    })
}

/// 备份路径:用户数据目录下 `xhup_backup/<文件名>`。
///
/// 保留策略(确定性、有界):每次 install/repair 都把将被覆盖的文件
/// 的**紧邻前一版本**写入备份(覆盖旧备份)。因此 `xhup_backup/` 至多
/// 含 OWNED_FILES 数量个文件,内容始终是「最近一次更新前的状态」,
/// 可直接手动回滚;不会无限累积历史备份目录。备份只含 XHUP 拥有
/// 文件,绝不触碰用户数据。
fn backup_path(user_data_dir: &Path, file: &str) -> PathBuf {
    user_data_dir.join("xhup_backup").join(file)
}

/// 把目标文件写入同目录隐藏临时文件(staging,不触碰最终产物)。
fn stage_file(user_data_dir: &Path, file: &str, contents: &str) -> Result<PathBuf, ManagerError> {
    let temporary = user_data_dir.join(format!(".{file}.xhup-tmp"));
    fs::write(&temporary, contents).map_err(|source| ManagerError::Io {
        path: temporary.clone(),
        source,
    })?;
    Ok(temporary)
}

/// 取计划动作所需的源包内容(缺包或缺文件都返回可操作错误)。
fn package_contents<'a>(
    package: Option<&'a RimePackage>,
    file: &str,
) -> Result<&'a str, ManagerError> {
    match package {
        None => Err(ManagerError::PackageMissing),
        Some(package) => package
            .contents_of(file)
            .ok_or_else(|| ManagerError::SourceMissing {
                file: file.to_string(),
            }),
    }
}

/// 已提交动作的回滚记录(Write 提交后删除目标;Overwrite 提交后从
/// 备份恢复)。
#[derive(Debug)]
struct Committed {
    file: String,
    had_backup: bool,
}

/// 校验计划只涉及 XHUP 拥有文件(execute 的唯一信任边界)。
///
/// - `file` 必须逐字出现在 [`OWNED_FILES`] 中(拒绝路径分隔符、绝对
///   路径与 `..` 逃逸;`Path::join` 遇绝对路径会替换基目录,必须在此
///   堵死);
/// - Overwrite 的 `backup` 必须与本目录推导的 [`backup_path`] 一致
///   (不信任计划携带的任意目录);
/// - 违规返回 [`ManagerError::PackageInvalid`]。
fn validate_plan_actions(plan: &Plan, user_data_dir: &Path) -> Result<(), ManagerError> {
    for action in &plan.actions {
        let file = action.file();
        if !OWNED_FILES.contains(&file) {
            return Err(ManagerError::PackageInvalid {
                missing: format!("非法计划目标:{file}"),
            });
        }
        if let PlanAction::Overwrite { backup, .. } = action
            && backup.as_path() != backup_path(user_data_dir, file)
        {
            return Err(ManagerError::PackageInvalid {
                missing: format!("非法备份路径:{}", backup.display()),
            });
        }
    }
    Ok(())
}

/// 要求路径是普通文件(拒绝符号链接,防止备份阶段把链接目标读入
/// xhup_backup 造成任意文件读取)。
fn require_regular_file(path: &Path) -> Result<(), ManagerError> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => Ok(()),
        Ok(_) => Err(ManagerError::Io {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "路径不是普通文件(可能是符号链接)",
            ),
        }),
        Err(source) => Err(ManagerError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// 回滚已提交动作(仅限拥有文件;尽力而为,返回第一个失败)。
fn rollback(committed: &[Committed], user_data_dir: &Path) -> Result<(), ManagerError> {
    for entry in committed {
        let target = user_data_dir.join(&entry.file);
        let result = if entry.had_backup {
            fs::copy(backup_path(user_data_dir, &entry.file), &target)
                .map(|_| ())
                .map_err(|source| ManagerError::Io {
                    path: target.clone(),
                    source,
                })
        } else {
            fs::remove_file(&target).map_err(|source| ManagerError::Io {
                path: target.clone(),
                source,
            })
        };
        result?;
    }
    Ok(())
}

/// 执行计划(把 dry-run 变成真实改动)。
///
/// install/repair(Write/Overwrite)按**事务性三阶段**执行,范围严格
/// 限于 XHUP 拥有文件:
///
/// 1. **staging**:全部新内容写入同目录 `.{file}.xhup-tmp` 临时文件;
///    任何一处失败则清理临时文件、不触碰任何最终产物;
/// 2. **backup**:每个 Overwrite 目标先复制到 `xhup_backup/`(内容为
///    紧邻前一版本,见 [`backup_path`] 保留策略);
/// 3. **commit**:逐个 rename 临时文件到最终位置;若中途失败,对已
///    提交动作执行回滚(Overwrite 从备份恢复,Write 删除),再返回
///    原始错误。
///
/// Delete(卸载)逐文件删除且幂等:文件已不存在视为成功;部分卸载
/// 可安全重跑,不做事务。
///
/// 返回完成的动作数。
pub fn execute(
    plan: &Plan,
    user_data_dir: &Path,
    package: Option<&RimePackage>,
) -> Result<usize, ManagerError> {
    // 信任边界:计划只能涉及拥有文件、备份只能在 xhup_backup/ 内。
    validate_plan_actions(plan, user_data_dir)?;
    let mut done = 0;
    // 卸载:逐文件删除(幂等)。
    let deletes: Vec<&PlanAction> = plan
        .actions
        .iter()
        .filter(|action| matches!(action, PlanAction::Delete { .. }))
        .collect();
    if !deletes.is_empty() {
        for action in &deletes {
            if let PlanAction::Delete { file } = action {
                let target = user_data_dir.join(file);
                if target.exists() {
                    fs::remove_file(&target).map_err(|source| ManagerError::Io {
                        path: target.clone(),
                        source,
                    })?;
                }
                done += 1;
            }
        }
        return Ok(done);
    }

    // 1. staging:全部写临时文件;失败则清理,不留任何最终产物改动。
    let mut staged: Vec<(String, PathBuf)> = Vec::new();
    let staging_result = (|| -> Result<(), ManagerError> {
        for action in &plan.actions {
            match action {
                PlanAction::Write { file } | PlanAction::Overwrite { file, .. } => {
                    let contents = package_contents(package, file)?;
                    let temporary = stage_file(user_data_dir, file, contents)?;
                    staged.push((file.clone(), temporary));
                }
                PlanAction::Delete { .. } => {}
            }
        }
        Ok(())
    })();
    if let Err(error) = staging_result {
        for (_, temporary) in &staged {
            let _ = fs::remove_file(temporary);
        }
        return Err(error);
    }

    // 2. backup:Overwrite 目标复制到 xhup_backup/(紧邻前一版本)。
    //    复制前要求普通文件(拒绝符号链接)。
    let backup_result = (|| -> Result<(), ManagerError> {
        for action in &plan.actions {
            if let PlanAction::Overwrite { file, backup } = action {
                let target = user_data_dir.join(file);
                require_regular_file(&target)?;
                if let Some(parent) = backup.parent() {
                    fs::create_dir_all(parent).map_err(|source| ManagerError::Io {
                        path: parent.to_path_buf(),
                        source,
                    })?;
                }
                fs::copy(&target, backup).map_err(|source| ManagerError::Io {
                    path: backup.clone(),
                    source,
                })?;
            }
        }
        Ok(())
    })();
    if let Err(error) = backup_result {
        for (_, temporary) in &staged {
            let _ = fs::remove_file(temporary);
        }
        return Err(error);
    }

    // 3. commit:逐个 rename;失败则回滚已提交动作。
    let mut committed: Vec<Committed> = Vec::new();
    for action in &plan.actions {
        match action {
            PlanAction::Write { file } | PlanAction::Overwrite { file, .. } => {
                let temporary = user_data_dir.join(format!(".{file}.xhup-tmp"));
                let target = user_data_dir.join(file);
                let committed_result = fs::rename(&temporary, &target).map_err(|source| {
                    let _ = fs::remove_file(&temporary);
                    ManagerError::Io {
                        path: target.clone(),
                        source,
                    }
                });
                if let Err(error) = committed_result {
                    let _ = rollback(&committed, user_data_dir);
                    return Err(error);
                }
                committed.push(Committed {
                    file: file.clone(),
                    had_backup: matches!(action, PlanAction::Overwrite { .. }),
                });
                done += 1;
            }
            PlanAction::Delete { .. } => {}
        }
    }
    Ok(done)
}

/// 学习数据摘要(不含任何学习内容)。
#[derive(Clone, Debug, Serialize)]
pub struct LearningSummary {
    /// 用户词典名(稳定兼容接口)。
    pub user_dict: String,
    /// 用户词典 DB 是否存在(是否已有学习数据)。
    pub db_exists: bool,
    /// 是否已有可恢复的导出快照。
    pub snapshot_available: bool,
    /// 学习管理工具(rime_dict_manager)是否可用;不可用时导出/导入
    /// 会返回可操作错误。
    pub tool_available: bool,
}

/// 快照文件名(`rime_dict_manager` 机械派生:`<词典名>.userdb.txt`)。
fn snapshot_filename() -> String {
    format!("{}.userdb.txt", xhup_cli::learning::FLOW_USER_DICT_NAME)
}

/// 在用户数据目录的 sync 树里查找快照(与 `xhup-cli` learning 模块的
/// 快照布局一致:`sync/<user_id>/<词典名>.userdb.txt`)。
fn find_snapshot(user_data_dir: &Path) -> Option<PathBuf> {
    let sync_dir = user_data_dir.join("sync");
    let entries = fs::read_dir(&sync_dir).ok()?;
    for entry in entries.flatten() {
        let Ok(files) = fs::read_dir(entry.path()) else {
            continue;
        };
        for file in files.flatten() {
            if file.file_name().to_string_lossy() == snapshot_filename() {
                return Some(file.path());
            }
        }
    }
    None
}

/// 汇总学习数据状态(纯文件系统 + PATH 检测,不调用外部工具,
/// 不输出学习内容)。
pub fn learning_summary(user_data_dir: &Path) -> LearningSummary {
    let db_path = user_data_dir.join(format!(
        "{}.userdb",
        xhup_cli::learning::FLOW_USER_DICT_NAME
    ));
    LearningSummary {
        user_dict: xhup_cli::learning::FLOW_USER_DICT_NAME.to_string(),
        db_exists: db_path.is_dir(),
        snapshot_available: find_snapshot(user_data_dir).is_some(),
        tool_available: xhup_cli::learning::which_dict_manager().is_some(),
    }
}

/// 生成脱敏诊断报告:版本/平台/架构/客户端/文件计数与完整性/方案/
/// 学习数据存在性/学习工具/重新部署能力;不包含学习词内容、用户
/// 其它文件、环境变量与凭据。用户数据目录路径属必要信息予以保留。
pub fn diagnostics_report(
    status: &InstallStatus,
    bundled_version: &str,
    learning: &LearningSummary,
) -> String {
    let mut report = String::new();
    report.push_str("XHUP Flow 诊断报告\n");
    report.push_str("==================\n");
    report.push_str(&format!("桌面应用版本: {bundled_version}\n"));
    report.push_str(&format!(
        "平台: {} / {}\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    report.push_str(&format!("客户端: {status:?}\n"));
    report.push_str(&format!(
        "XHUP 文件: {}/{}\n",
        status.installed_files,
        OWNED_FILES.len()
    ));
    report.push_str("已安装版本: ");
    report.push_str(status.installed_version.as_deref().unwrap_or("(未安装)"));
    report.push('\n');
    report.push_str(&format!("方案: {}\n", status.schemas.join(", ")));
    if status.missing_files.is_empty() {
        report.push_str("缺失文件: 无\n");
    } else {
        report.push_str(&format!("缺失文件: {}\n", status.missing_files.join(", ")));
    }
    let matches = status
        .integrity
        .iter()
        .filter(|s| **s == FileIntegrity::Match)
        .count();
    let different = status
        .integrity
        .iter()
        .filter(|s| **s == FileIntegrity::Different)
        .count();
    report.push_str(&format!("完整性: 一致 {matches} / 不同 {different}\n"));
    report.push_str(&format!(
        "学习数据: {}\n",
        if learning.db_exists {
            "已有本地 userdb"
        } else {
            "尚无学习数据"
        }
    ));
    report.push_str(&format!(
        "学习管理工具: {}\n",
        if learning.tool_available {
            "可用"
        } else {
            "未找到 rime_dict_manager(导出/导入不可用)"
        }
    ));
    report.push_str(&format!(
        "重新部署: 手动执行({})\n",
        status.client.redeploy_guidance()
    ));
    report.push_str("隐私: 学习数据仅存本机;本报告不含任何用户词内容与环境变量。\n");
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xhup-manager-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 构造一个合法内存源包(11 个拥有文件,内容带标记)。
    fn fake_package(marker: &str) -> RimePackage {
        let schema = format!("{FLOW_SCHEMA_ID}.schema.yaml");
        let files = OWNED_FILES
            .iter()
            .map(|file| {
                let contents = if *file == schema {
                    format!("schema:\n  version: \"{marker}\"\n")
                } else {
                    marker.to_string()
                };
                ((*file).to_string(), contents)
            })
            .collect();
        RimePackage {
            version: marker.to_string(),
            files,
        }
    }

    /// 构造一个「装了无关用户文件」的假 Rime 用户目录。
    fn fake_user_dir(name: &str) -> PathBuf {
        let dir = temp_dir(name);
        fs::write(dir.join("default.custom.yaml"), "用户的自定义配置").unwrap();
        fs::create_dir_all(dir.join("xhup_flow_user.userdb")).unwrap();
        dir
    }

    #[test]
    fn bundled_package_matches_ownership_manifest() {
        let package = RimePackage::bundled().unwrap();
        for file in OWNED_FILES {
            assert!(
                package.contents_of(file).is_some(),
                "生成器产物缺少拥有文件 {file}"
            );
        }
        assert_eq!(package.files.len(), OWNED_FILES.len());
        assert!(!package.version.is_empty());
    }

    #[test]
    fn parse_schema_version_handles_quotes_and_missing() {
        assert_eq!(
            parse_schema_version("schema:\n  version: \"1.2.3\"\n").as_deref(),
            Some("1.2.3")
        );
        assert!(parse_schema_version("schema:\n  name: x\n").is_none());
    }

    #[test]
    fn plan_install_requires_valid_user_dir_and_package() {
        let missing = temp_dir("nope");
        let _ = fs::remove_dir_all(&missing);
        let user = fake_user_dir("plan-bad-user");
        let package = fake_package("1.0.0");
        assert!(matches!(
            plan_install(&missing, &package),
            Err(ManagerError::UserDataDirMissing { .. })
        ));
        // 缺文件的包 → 拒绝。
        let mut broken = package.clone();
        broken.files.remove(0);
        assert!(matches!(
            plan_install(&user, &broken),
            Err(ManagerError::PackageInvalid { .. })
        ));
        let _ = fs::remove_dir_all(&user);
    }

    #[test]
    fn fresh_install_writes_all_owned_files_and_preserves_user_files() {
        let user = fake_user_dir("fresh-install");
        let package = fake_package("1.0.0");
        let plan = plan_install(&user, &package).unwrap();
        assert_eq!(plan.actions.len(), OWNED_FILES.len());
        assert!(
            plan.actions
                .iter()
                .all(|action| matches!(action, PlanAction::Write { .. }))
        );
        assert_eq!(
            execute(&plan, &user, Some(&package)).unwrap(),
            OWNED_FILES.len()
        );
        for file in OWNED_FILES {
            assert!(user.join(file).is_file(), "{file} 应被安装");
        }
        // 无关用户文件与 userdb 必须原样保留。
        assert!(user.join("default.custom.yaml").is_file());
        assert!(user.join("xhup_flow_user.userdb").is_dir());
        // 安装后状态含已安装版本。
        let status = install_status(&user, RimeClient::Fcitx5, Some(&package));
        assert_eq!(status.installed_version.as_deref(), Some("1.0.0"));
        assert_eq!(status.schemas, vec![FLOW_SCHEMA_ID, STATIC_SCHEMA_ID]);
        let _ = fs::remove_dir_all(&user);
    }

    #[test]
    fn upgrade_overwrites_with_backup_and_repair_writes_missing() {
        let user = fake_user_dir("upgrade");
        let package_v1 = fake_package("1.0.0");
        let plan_v1 = plan_install(&user, &package_v1).unwrap();
        execute(&plan_v1, &user, Some(&package_v1)).unwrap();

        // 升级:v2 内容 → 全部 Overwrite,且旧内容备份在 xhup_backup/。
        let package_v2 = fake_package("1.1.0");
        let plan_v2 = plan_install(&user, &package_v2).unwrap();
        assert!(
            plan_v2
                .actions
                .iter()
                .all(|action| matches!(action, PlanAction::Overwrite { .. }))
        );
        execute(&plan_v2, &user, Some(&package_v2)).unwrap();
        assert_eq!(
            fs::read_to_string(user.join(OWNED_FILES[0])).unwrap(),
            "1.1.0"
        );
        assert_eq!(
            fs::read_to_string(backup_path(&user, OWNED_FILES[0])).unwrap(),
            "1.0.0"
        );
        assert_eq!(
            install_status(&user, RimeClient::Fcitx5, Some(&package_v2))
                .installed_version
                .as_deref(),
            Some("1.1.0")
        );

        // 修复:删掉一个文件 → 下一计划中该文件是 Write。
        fs::remove_file(user.join(OWNED_FILES[3])).unwrap();
        let plan_repair = plan_install(&user, &package_v2).unwrap();
        assert!(plan_repair.actions.iter().any(|action| matches!(
            action,
            PlanAction::Write { file } if file == OWNED_FILES[3]
        )));
        let status = install_status(&user, RimeClient::Fcitx5, Some(&package_v2));
        assert_eq!(status.missing_files.len(), 1);
        execute(&plan_repair, &user, Some(&package_v2)).unwrap();
        assert_eq!(
            install_status(&user, RimeClient::Fcitx5, Some(&package_v2))
                .missing_files
                .len(),
            0
        );

        let _ = fs::remove_dir_all(&user);
    }

    #[test]
    fn uninstall_deletes_only_owned_files() {
        let user = fake_user_dir("uninstall");
        let package = fake_package("1.0.0");
        execute(
            &plan_install(&user, &package).unwrap(),
            &user,
            Some(&package),
        )
        .unwrap();
        let plan = plan_uninstall(&user).unwrap();
        assert_eq!(plan.actions.len(), OWNED_FILES.len());
        execute(&plan, &user, None).unwrap();
        for file in OWNED_FILES {
            assert!(!user.join(file).exists(), "{file} 应被删除");
        }
        // 无关文件与 userdb 保留。
        assert!(user.join("default.custom.yaml").is_file());
        assert!(user.join("xhup_flow_user.userdb").is_dir());
        // 再卸载 → 空计划(幂等)。
        assert!(plan_uninstall(&user).unwrap().is_empty());
        let _ = fs::remove_dir_all(&user);
    }

    #[test]
    fn dry_run_plan_does_not_touch_disk_until_executed() {
        let user = fake_user_dir("dry-run");
        let package = fake_package("1.0.0");
        let plan = plan_install(&user, &package).unwrap();
        // 计划产出后,盘上没有任何 XHUP 文件。
        for file in OWNED_FILES {
            assert!(!user.join(file).exists());
        }
        execute(&plan, &user, Some(&package)).unwrap();
        for file in OWNED_FILES {
            assert!(user.join(file).is_file());
        }
        let _ = fs::remove_dir_all(&user);
    }

    #[test]
    fn execute_without_package_fails_cleanly_for_writes() {
        let user = fake_user_dir("no-package");
        let package = fake_package("1.0.0");
        let plan = plan_install(&user, &package).unwrap();
        assert!(matches!(
            execute(&plan, &user, None),
            Err(ManagerError::PackageMissing)
        ));
        // 失败后不留下半成品目标文件(临时文件已清理)。
        for file in OWNED_FILES {
            assert!(!user.join(file).exists());
            assert!(!user.join(format!(".{file}.xhup-tmp")).exists());
        }
        let _ = fs::remove_dir_all(&user);
    }

    #[test]
    fn learning_summary_reports_fs_state_without_tool() {
        let user = fake_user_dir("learning");
        let summary = learning_summary(&user);
        assert_eq!(summary.user_dict, "xhup_flow_user");
        assert!(summary.db_exists);
        assert!(!summary.snapshot_available);
        // 快照出现后可被识别。
        let sync = user.join("sync").join("unknown");
        fs::create_dir_all(&sync).unwrap();
        fs::write(sync.join(snapshot_filename()), "# 快照").unwrap();
        assert!(learning_summary(&user).snapshot_available);
        let _ = fs::remove_dir_all(&user);
    }

    /// 版本同步守卫:工作区版本与 Tauri 应用版本必须一致,防止发布
    /// 版本漂移(tauri.conf.json 驱动桌面安装包,workspace 驱动
    /// Rime 包内嵌版本,二者必须同步修改)。
    #[test]
    fn product_versions_are_synchronized() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace = fs::read_to_string(manifest_dir.join("../../Cargo.toml")).unwrap();
        let workspace_version = workspace
            .lines()
            .find_map(|line| {
                let trimmed = line.trim();
                trimmed
                    .strip_prefix("version = \"")
                    .map(|rest| rest.trim_end_matches('"').to_string())
            })
            .expect("workspace Cargo.toml 应包含 workspace version");
        let conf = fs::read_to_string(manifest_dir.join("tauri.conf.json")).unwrap();
        let conf: serde_json::Value = serde_json::from_str(&conf).unwrap();
        let app_version = conf["version"]
            .as_str()
            .expect("tauri.conf.json 应包含 version");
        assert_eq!(
            workspace_version, app_version,
            "workspace 与 tauri.conf.json 版本漂移"
        );
    }

    #[test]
    fn diagnostics_report_is_sanitized() {
        let user = fake_user_dir("diag");
        let package = fake_package("1.0.0");
        execute(
            &plan_install(&user, &package).unwrap(),
            &user,
            Some(&package),
        )
        .unwrap();
        let status = install_status(&user, RimeClient::Fcitx5, Some(&package));
        let learning = learning_summary(&user);
        let report = diagnostics_report(&status, "0.1.0", &learning);
        assert!(report.contains("XHUP 文件: 11/11"));
        assert!(report.contains("xhup_flow, xhup_flow_static"));
        assert!(report.contains("已安装版本: 1.0.0"));
        assert!(report.contains("完整性: 一致 11 / 不同 0"));
        assert!(report.contains("平台: "));
        assert!(report.contains("重新部署: 手动执行("));
        assert!(
            !report.contains("default.custom.yaml"),
            "不包含用户文件内容"
        );
        let _ = fs::remove_dir_all(&user);
    }

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(
            ManagerError::UserDataDirMissing {
                path: PathBuf::from("/x")
            }
            .code(),
            "user_data_dir_missing"
        );
        assert_eq!(ManagerError::PackageMissing.code(), "package_missing");
        assert_eq!(
            ManagerError::PackageInvalid {
                missing: "a".to_string()
            }
            .code(),
            "package_invalid"
        );
    }

    #[test]
    fn repeated_updates_refresh_backup_to_previous_version() {
        // 备份保留策略:备份内容 = 紧邻本次更新前的版本(有界、确定性)。
        let user = fake_user_dir("backup-retention");
        let v1 = fake_package("1.0.0");
        execute(&plan_install(&user, &v1).unwrap(), &user, Some(&v1)).unwrap();
        let v2 = fake_package("1.1.0");
        execute(&plan_install(&user, &v2).unwrap(), &user, Some(&v2)).unwrap();
        let v3 = fake_package("1.2.0");
        execute(&plan_install(&user, &v3).unwrap(), &user, Some(&v3)).unwrap();
        // 第二次更新后,备份 = v2(紧邻前一版本),不是更早的 v1。
        assert_eq!(
            fs::read_to_string(backup_path(&user, OWNED_FILES[0])).unwrap(),
            "1.1.0"
        );
        // 备份目录文件数有界(恰好拥有文件数)。
        let count = fs::read_dir(user.join("xhup_backup")).unwrap().count();
        assert_eq!(count, OWNED_FILES.len());
        let _ = fs::remove_dir_all(&user);
    }

    #[test]
    fn staging_failure_aborts_without_touching_any_target() {
        // 失败注入:一个目标是目录 → staging 之前的备份阶段失败...
        let user = fake_user_dir("stage-abort");
        let v1 = fake_package("1.0.0");
        execute(&plan_install(&user, &v1).unwrap(), &user, Some(&v1)).unwrap();
        // 把一个已安装文件替换成目录,使其备份(copy)失败。
        let victim = user.join(OWNED_FILES[5]);
        fs::remove_file(&victim).unwrap();
        fs::create_dir(&victim).unwrap();
        let v2 = fake_package("1.1.0");
        let result = execute(&plan_install(&user, &v2).unwrap(), &user, Some(&v2));
        assert!(result.is_err());
        // 其它拥有文件必须保持 v1 原样(任何最终产物都未被改动)。
        assert_eq!(
            fs::read_to_string(user.join(OWNED_FILES[0])).unwrap(),
            "1.0.0"
        );
        assert_eq!(
            fs::read_to_string(user.join(OWNED_FILES[2])).unwrap(),
            "1.0.0"
        );
        // 不残留 staging 临时文件。
        for file in OWNED_FILES {
            assert!(!user.join(format!(".{file}.xhup-tmp")).exists());
        }
        let _ = fs::remove_dir_all(&user);
    }

    #[test]
    fn rollback_restores_previous_state_from_commit_log() {
        // 直接验证回滚:一半 Write、一半 Overwrite 的已提交记录。
        let user = fake_user_dir("rollback");
        let v1 = fake_package("1.0.0");
        execute(&plan_install(&user, &v1).unwrap(), &user, Some(&v1)).unwrap();
        // 前 6 个文件视为 Overwrite 提交(应从备份恢复),后 5 个视为
        // Write 提交(应被删除)。构造备份与「新版本」内容。
        for file in &OWNED_FILES[..6] {
            let backup = backup_path(&user, file);
            fs::create_dir_all(backup.parent().unwrap()).unwrap();
            fs::copy(user.join(file), &backup).unwrap();
            fs::write(user.join(file), "v2").unwrap();
        }
        for file in &OWNED_FILES[6..] {
            fs::write(user.join(file), "v2").unwrap();
        }
        let committed: Vec<Committed> = OWNED_FILES
            .iter()
            .enumerate()
            .map(|(index, file)| Committed {
                file: (*file).to_string(),
                had_backup: index < 6,
            })
            .collect();
        rollback(&committed, &user).unwrap();
        // Overwrite → 恢复为 v1;Write → 删除。
        assert_eq!(
            fs::read_to_string(user.join(OWNED_FILES[0])).unwrap(),
            "1.0.0"
        );
        assert!(!user.join(OWNED_FILES[8]).exists());
        let _ = fs::remove_dir_all(&user);
    }

    #[test]
    fn integrity_and_health_classification() {
        let user = fake_user_dir("integrity");
        let v1 = fake_package("1.0.0");
        execute(&plan_install(&user, &v1).unwrap(), &user, Some(&v1)).unwrap();

        // 与同版本包比对 → 全部 Match → Healthy。
        let status = install_status(&user, RimeClient::Fcitx5, Some(&v1));
        assert!(status.integrity.iter().all(|s| *s == FileIntegrity::Match));
        assert_eq!(status.health(&v1.version), InstallHealth::Healthy);

        // 修改一个文件内容(不换版本号)→ Different → Modified。
        fs::write(user.join(OWNED_FILES[2]), "被外部改动").unwrap();
        let status = install_status(&user, RimeClient::Fcitx5, Some(&v1));
        assert!(status.integrity.contains(&FileIntegrity::Different));
        assert_eq!(status.health(&v1.version), InstallHealth::Modified);
        fs::write(
            user.join(OWNED_FILES[2]),
            v1.contents_of(OWNED_FILES[2]).unwrap(),
        )
        .unwrap();

        // 新版本包比对 → 版本不同 → UpdateAvailable(内容一致时)。
        let v2 = fake_package("1.1.0");
        let status = install_status(&user, RimeClient::Fcitx5, Some(&v2));
        // v1 与 v2 除 schema 版本行外内容一致:主方案文件 Different,
        // 但已安装版本 != 随附版本 → UpdateAvailable 优先于 Modified。
        assert!(status.integrity.contains(&FileIntegrity::Different));
        assert_eq!(status.health(&v2.version), InstallHealth::UpdateAvailable);

        // 缺文件 → Incomplete 优先于其它分类。
        fs::remove_file(user.join(OWNED_FILES[4])).unwrap();
        let status = install_status(&user, RimeClient::Fcitx5, Some(&v1));
        assert_eq!(status.health(&v1.version), InstallHealth::Incomplete);

        // 空目录 → NotInstalled。
        let empty = temp_dir("integrity-empty");
        let status = install_status(&empty, RimeClient::Fcitx5, Some(&v1));
        assert_eq!(status.health(&v1.version), InstallHealth::NotInstalled);
        let _ = fs::remove_dir_all(&user);
        let _ = fs::remove_dir_all(&empty);
    }

    #[test]
    fn execute_rejects_plans_targeting_non_owned_files() {
        // 信任边界:伪造计划(绝对路径/未知文件/非法备份目录)必须被
        // 拒绝,不得触碰用户目录。
        let user = fake_user_dir("plan-trust");
        let package = fake_package("1.0.0");
        let mut hostile = plan_install(&user, &package).unwrap();
        hostile.actions[0] = PlanAction::Write {
            file: "../evil.yaml".to_string(),
        };
        assert!(matches!(
            execute(&hostile, &user, Some(&package)),
            Err(ManagerError::PackageInvalid { .. })
        ));
        // 未知文件名。
        hostile.actions[0] = PlanAction::Write {
            file: "/etc/passwd".to_string(),
        };
        assert!(matches!(
            execute(&hostile, &user, Some(&package)),
            Err(ManagerError::PackageInvalid { .. })
        ));
        // 非法备份目录(不在 xhup_backup/ 内)。
        let mut hostile_backup = plan_install(&user, &package).unwrap();
        hostile_backup.actions[0] = PlanAction::Overwrite {
            file: OWNED_FILES[0].to_string(),
            backup: user.join("elsewhere").join(OWNED_FILES[0]),
        };
        assert!(matches!(
            execute(&hostile_backup, &user, Some(&package)),
            Err(ManagerError::PackageInvalid { .. })
        ));
        // 用户目录未被改动。
        assert!(!user.join(OWNED_FILES[0]).exists());
        let _ = fs::remove_dir_all(&user);
    }

    #[test]
    fn execute_rejects_symlink_targets_during_backup() {
        // 符号链接防护:备份阶段拒绝链接目标,避免任意文件读入备份。
        let user = fake_user_dir("symlink");
        let package = fake_package("1.0.0");
        execute(
            &plan_install(&user, &package).unwrap(),
            &user,
            Some(&package),
        )
        .unwrap();
        // 把一个拥有文件替换为指向外部文件的符号链接。
        let outside = temp_dir("symlink-outside");
        fs::write(outside.join("secret"), "机密").unwrap();
        let victim = user.join(OWNED_FILES[0]);
        fs::remove_file(&victim).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.join("secret"), &victim).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(outside.join("secret"), &victim).unwrap();
        let v2 = fake_package("1.1.0");
        let result = execute(&plan_install(&user, &v2).unwrap(), &user, Some(&v2));
        assert!(result.is_err());
        // 链接目标内容未被读入备份目录。
        let leaked = user.join("xhup_backup").join(OWNED_FILES[0]);
        assert!(!leaked.exists() || fs::read_to_string(&leaked).unwrap() != "机密");
        let _ = fs::remove_dir_all(&user);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn redeploy_support_is_capability_based_with_injected_probe() {
        // 官方机制的参数构造(源码级验证,与环境无关):
        // - Fcitx5:dbus-send 调用官方 D-Bus 接口
        //   SetConfig fcitx://config/addon/rime/deploy(进程内部署);
        let fcitx_args = RimeClient::Fcitx5.redeploy_arguments();
        assert!(
            fcitx_args
                .iter()
                .any(|arg| arg.contains("fcitx://config/addon/rime/deploy"))
        );
        assert!(fcitx_args.contains(&"org.fcitx.Fcitx.Controller1.SetConfig"));
        // - Ibus:`ibus restart`(官方 CLI,不杀任意进程);
        assert_eq!(RimeClient::Ibus.redeploy_arguments(), &["restart"]);
        // - Squirrel:`--reload`(向运行中进程投递部署通知);
        assert_eq!(RimeClient::Squirrel.redeploy_arguments(), &["--reload"]);
        // - Weasel:参数必须恰好为 /deploy(源码 wcscmp 全行匹配)。
        assert_eq!(RimeClient::Weasel.redeploy_arguments(), &["/deploy"]);

        // 注入假候选 → Automatic,程序与参数按候选原样传递。
        let fake_dir = temp_dir("redeploy-fake");
        let fake = fake_dir.join("dbus-send");
        fs::write(&fake, "#!/bin/sh\nexit 0\n").unwrap();
        let candidates = vec![(fake.clone(), vec!["--session".to_string()])];
        match resolve_redeploy(&candidates, &|path| *path == fake) {
            RedeploySupport::Automatic { program, args } => {
                assert_eq!(program, fake);
                assert_eq!(args, vec![std::ffi::OsString::from("--session")]);
            }
            RedeploySupport::Manual => panic!("假可执行文件应被探测为 Automatic"),
        }
        // 全部候选缺失 → Manual(能力型 UI 显示手动指引)。
        assert!(matches!(
            resolve_redeploy(&candidates, &|_| false),
            RedeploySupport::Manual
        ));
        let _ = fs::remove_dir_all(&fake_dir);
    }

    #[test]
    fn fcitx5_user_data_dir_prefers_xdg_data_home() {
        // 源码级修正:fcitx5-rime 用户目录是 $XDG_DATA_HOME/fcitx5/rime。
        // 安全性:仅本测试读写这两个变量,其余测试不读 XDG_*。
        let dir = temp_dir("xdg-probe");
        // SAFETY:测试进程内独占环境变量(其余测试不读 XDG_*)。
        unsafe {
            std::env::set_var("XDG_DATA_HOME", &dir);
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        let candidate = RimeClient::Fcitx5.default_user_data_dir().unwrap();
        assert_eq!(candidate, dir.join("fcitx5/rime"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn e2e_lifecycle_install_to_uninstall_preserves_user_data() {
        // 全生命周期:全新安装 → Healthy → 改动 → Modified → 修复 →
        // 升级 → 学习数据保留 → 卸载 → 无关文件与 userdb 保留。
        let user = fake_user_dir("e2e");
        let v1 = fake_package("1.0.0");

        // 全新安装 → Healthy。
        execute(&plan_install(&user, &v1).unwrap(), &user, Some(&v1)).unwrap();
        let status = install_status(&user, RimeClient::Fcitx5, Some(&v1));
        assert_eq!(status.health(&v1.version), InstallHealth::Healthy);

        // 外部改动 → Modified。
        fs::write(user.join(OWNED_FILES[2]), "改动").unwrap();
        let status = install_status(&user, RimeClient::Fcitx5, Some(&v1));
        assert_eq!(status.health(&v1.version), InstallHealth::Modified);

        // 修复 → 回到 Healthy(内容恢复)。
        execute(&plan_install(&user, &v1).unwrap(), &user, Some(&v1)).unwrap();
        assert_eq!(
            install_status(&user, RimeClient::Fcitx5, Some(&v1)).health(&v1.version),
            InstallHealth::Healthy
        );

        // 升级 → 新版本 Healthy;userdb 仍在。
        let v2 = fake_package("1.1.0");
        execute(&plan_install(&user, &v2).unwrap(), &user, Some(&v2)).unwrap();
        assert_eq!(
            install_status(&user, RimeClient::Fcitx5, Some(&v2)).health(&v2.version),
            InstallHealth::Healthy
        );
        assert!(user.join("xhup_flow_user.userdb").is_dir());

        // 卸载 → 拥有文件全删,无关文件与 userdb 保留,幂等。
        execute(&plan_uninstall(&user).unwrap(), &user, None).unwrap();
        for file in OWNED_FILES {
            assert!(!user.join(file).exists());
        }
        assert!(user.join("default.custom.yaml").is_file());
        assert!(user.join("xhup_flow_user.userdb").is_dir());
        assert!(plan_uninstall(&user).unwrap().is_empty());
        let _ = fs::remove_dir_all(&user);
    }
}
