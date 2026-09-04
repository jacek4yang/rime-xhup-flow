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
    pub fn default_user_data_dir(&self) -> Option<PathBuf> {
        match self {
            Self::Weasel => {
                std::env::var_os("APPDATA").map(|base| PathBuf::from(base).join("Rime"))
            }
            Self::Squirrel => {
                std::env::var_os("HOME").map(|base| PathBuf::from(base).join("Library/Rime"))
            }
            Self::Fcitx5 => std::env::var_os("XDG_CONFIG_HOME")
                .map(|base| PathBuf::from(base).join("fcitx5/rime"))
                .or_else(|| {
                    std::env::var_os("HOME")
                        .map(|base| PathBuf::from(base).join(".config/fcitx5/rime"))
                }),
            Self::Ibus => std::env::var_os("XDG_DATA_HOME")
                .map(|base| PathBuf::from(base).join("ibus/rime"))
                .or_else(|| {
                    std::env::var_os("HOME")
                        .map(|base| PathBuf::from(base).join(".config/ibus/rime"))
                }),
        }
    }

    /// 重新部署的操作指引(各平台机制不同,不做自动部署;不发明命令)。
    pub fn redeploy_guidance(&self) -> &'static str {
        match self {
            Self::Weasel => "在系统托盘的「小狼毫」菜单中执行「重新部署」。",
            Self::Squirrel => "在菜单栏「鼠须管」菜单中执行「重新部署」。",
            Self::Fcitx5 => "运行 fcitx5-rime 的「重新部署」,或重启 Fcitx5。",
            Self::Ibus => "运行 ibus restart,或在 IBus 设置中重新部署 Rime。",
        }
    }
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
    pub fn bundled() -> Self {
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
            .expect("bundled package must embed a schema version");
        Self { version, files }
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
}

/// 检查用户数据目录内的安装状态。
pub fn install_status(user_data_dir: &Path, client: RimeClient) -> InstallStatus {
    let mut installed_files = 0;
    let mut missing_files = Vec::new();
    for file in OWNED_FILES {
        if user_data_dir.join(file).is_file() {
            installed_files += 1;
        } else {
            missing_files.push((*file).to_string());
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

/// 备份路径:用户数据目录下 `xhup_backup/<文件名>`。备份已存在时跳过
/// (保留最早版本内容可回滚)。
fn backup_path(user_data_dir: &Path, file: &str) -> PathBuf {
    user_data_dir.join("xhup_backup").join(file)
}

/// 原子写入:先写同目录隐藏临时文件,再替换最终产物。失败时不破坏
/// 既有目标文件。
fn write_atomically(user_data_dir: &Path, file: &str, contents: &str) -> Result<(), ManagerError> {
    let target = user_data_dir.join(file);
    let temporary = user_data_dir.join(format!(".{file}.xhup-tmp"));
    fs::write(&temporary, contents).map_err(|source| ManagerError::Io {
        path: temporary.clone(),
        source,
    })?;
    fs::rename(&temporary, &target).map_err(|source| {
        let _ = fs::remove_file(&temporary);
        ManagerError::Io {
            path: target.clone(),
            source,
        }
    })
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

/// 执行计划(把 dry-run 变成真实改动)。
///
/// - Write / Overwrite 需要 `package` 提供文件内容;
/// - Overwrite 先把旧文件复制到备份路径(已存在则跳过);
/// - Delete 只删拥有文件;
/// - 返回完成的动作数。
pub fn execute(
    plan: &Plan,
    user_data_dir: &Path,
    package: Option<&RimePackage>,
) -> Result<usize, ManagerError> {
    let mut done = 0;
    for action in &plan.actions {
        match action {
            PlanAction::Write { file } => {
                let contents = package_contents(package, file)?;
                write_atomically(user_data_dir, file, contents)?;
            }
            PlanAction::Overwrite { file, backup } => {
                let contents = package_contents(package, file)?;
                if let Some(parent) = backup.parent() {
                    fs::create_dir_all(parent).map_err(|source| ManagerError::Io {
                        path: parent.to_path_buf(),
                        source,
                    })?;
                }
                let target = user_data_dir.join(file);
                if !backup.exists() {
                    fs::copy(&target, backup).map_err(|source| ManagerError::Io {
                        path: backup.clone(),
                        source,
                    })?;
                }
                write_atomically(user_data_dir, file, contents)?;
            }
            PlanAction::Delete { file } => {
                let target = user_data_dir.join(file);
                fs::remove_file(&target).map_err(|source| ManagerError::Io {
                    path: target.clone(),
                    source,
                })?;
            }
        }
        done += 1;
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

/// 生成脱敏诊断报告:版本/平台/文件计数/方案/学习数据存在性;
/// 不包含学习词内容、用户其它文件与环境细节。
pub fn diagnostics_report(
    status: &InstallStatus,
    bundled_version: &str,
    learning: &LearningSummary,
) -> String {
    let mut report = String::new();
    report.push_str("XHUP Flow 诊断报告\n");
    report.push_str("==================\n");
    report.push_str(&format!("桌面应用版本: {bundled_version}\n"));
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
    report.push_str(&format!(
        "学习数据: {}\n",
        if learning.db_exists {
            "已有本地 userdb"
        } else {
            "尚无学习数据"
        }
    ));
    if !learning.tool_available {
        report.push_str("学习管理工具: 未找到 rime_dict_manager(导出/导入不可用)\n");
    }
    report.push_str("隐私: 学习数据仅存本机;本报告不含任何用户词内容。\n");
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
        let package = RimePackage::bundled();
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
        let status = install_status(&user, RimeClient::Fcitx5);
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
            install_status(&user, RimeClient::Fcitx5)
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
        let status = install_status(&user, RimeClient::Fcitx5);
        assert_eq!(status.missing_files.len(), 1);
        execute(&plan_repair, &user, Some(&package_v2)).unwrap();
        assert_eq!(
            install_status(&user, RimeClient::Fcitx5)
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
        let status = install_status(&user, RimeClient::Fcitx5);
        let learning = learning_summary(&user);
        let report = diagnostics_report(&status, "0.1.0", &learning);
        assert!(report.contains("XHUP 文件: 11/11"));
        assert!(report.contains("xhup_flow, xhup_flow_static"));
        assert!(report.contains("已安装版本: 1.0.0"));
        assert!(
            !report.contains("default.custom.yaml"),
            "不包含用户文件内容"
        );
        let _ = fs::remove_dir_all(&user);
    }
}
