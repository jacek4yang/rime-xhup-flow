//! Tauri 命令层:控制中心 UI 与纯 Rust 管理核心(`manager`)之间的
//! 薄封装。
//!
//! 本模块不做业务决策:全部平台/安装/学习逻辑在 [`crate::manager`];
//! 这里只负责检测环境、调用核心、把结果序列化给前端。错误统一转为
//! [`CommandError`](稳定机器码 + 人读消息),前端按码翻译展示,
//! 绝不解析错误字符串。

use serde::Serialize;

use crate::manager::{
    self, InstallHealth, InstallStatus, LearningSummary, ManagerError, Plan, RimeClient,
    RimePackage,
};

/// 机器可读的命令错误(前端按 `code` 翻译;`message` 仅作兜底展示)。
#[derive(Debug, Serialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl CommandError {
    fn new(code: &str, message: String) -> Self {
        Self {
            code: code.to_string(),
            message,
        }
    }
}

impl From<ManagerError> for CommandError {
    fn from(error: ManagerError) -> Self {
        Self::new(error.code(), error.to_string())
    }
}

/// `xhup-cli` learning 错误 → 稳定错误码(学习管理是兼容接口,错误
/// 映射集中在这一处)。
fn learning_error(error: xhup_cli::learning::LearningError) -> CommandError {
    use xhup_cli::learning::LearningError as E;
    let code = match &error {
        E::DictManagerNotFound { .. } => "dict_manager_missing",
        E::UserDataDirMissing { .. } => "user_data_dir_missing",
        E::SnapshotMissing { .. } => "snapshot_missing",
        E::SnapshotNameMismatch { .. } => "snapshot_name_mismatch",
        E::SnapshotNotProduced { .. } => "snapshot_not_produced",
        E::UserDictAbsent => "user_dict_absent",
        E::ToolFailed { .. } => "tool_failed",
        E::ResetNotConfirmed => "reset_not_confirmed",
        E::ResetFailed { .. } => "reset_failed",
    };
    CommandError::new(code, error.to_string())
}

/// 控制中心首页的完整产品状态。
#[derive(Debug, Serialize)]
pub struct ProductStatus {
    /// 当前平台客户端。
    pub client: RimeClient,
    /// 重新部署指引(执行安装/升级后展示)。
    pub redeploy_guidance: String,
    /// 默认用户数据目录(平台已知的候选;可能不存在)。
    pub user_data_dir: Option<std::path::PathBuf>,
    /// Rime 用户数据目录是否已存在(视作「检测到 Rime 环境」)。
    pub rime_detected: bool,
    /// 安装状态(目录存在时才有)。
    pub install: Option<InstallStatus>,
    /// 桌面应用随附的源包版本。
    pub bundled_version: String,
    /// 已安装版本落后于随附包(需要升级)。
    pub update_available: bool,
    /// 安装健康分类。
    pub health: Option<InstallHealth>,
    /// 学习数据摘要。
    pub learning: Option<LearningSummary>,
}

/// 计划执行结果。
#[derive(Debug, Serialize)]
pub struct ExecuteResult {
    /// 完成的动作数。
    pub done: usize,
    /// 重新部署指引(安装/升级后需要用户重新部署)。
    pub redeploy_guidance: String,
}

/// 当前环境的用户数据目录;目录不存在时返回可操作错误。
fn require_user_data_dir() -> Result<(RimeClient, std::path::PathBuf), CommandError> {
    let (client, dir) = manager::detect_platform();
    let dir = dir.ok_or_else(|| {
        CommandError::new(
            "user_data_dir_unavailable",
            "无法确定本平台的 Rime 用户数据目录".to_string(),
        )
    })?;
    if !dir.is_dir() {
        return Err(CommandError::new(
            "rime_not_detected",
            format!(
                "Rime 用户数据目录不存在({}):请先安装对应的 Rime 客户端。",
                dir.display()
            ),
        ));
    }
    Ok((client, dir))
}

/// 读取控制中心完整产品状态。
#[tauri::command]
pub fn product_status() -> Result<ProductStatus, CommandError> {
    let (client, dir) = manager::detect_platform();
    let rime_detected = dir.as_deref().is_some_and(std::path::Path::is_dir);
    let package = RimePackage::bundled()?;
    let install = dir
        .as_deref()
        .filter(|dir| dir.is_dir())
        .map(|dir| manager::install_status(dir, client, Some(&package)));
    let update_available = match &install {
        Some(status) => match &status.installed_version {
            Some(installed) => *installed != package.version,
            None => false,
        },
        None => false,
    };
    let health = install.as_ref().map(|s| s.health(&package.version));
    let learning = dir
        .as_deref()
        .filter(|dir| dir.is_dir())
        .map(manager::learning_summary);
    Ok(ProductStatus {
        client,
        redeploy_guidance: client.redeploy_guidance().to_string(),
        user_data_dir: dir,
        rime_detected,
        install,
        bundled_version: package.version,
        update_available,
        health,
        learning,
    })
}

/// 产出维护计划(dry-run):`install` 覆盖安装/升级/修复(按当前状态
/// 自动决定 Write/Overwrite),`uninstall` 只列将删除的拥有文件。
/// 计划不落盘;确认后交 [`product_execute`]。
#[tauri::command]
pub fn product_plan(kind: &str) -> Result<Plan, CommandError> {
    let (_client, dir) = require_user_data_dir()?;
    let plan = match kind {
        "install" => {
            let package = RimePackage::bundled()?;
            manager::plan_install(&dir, &package)?
        }
        "uninstall" => manager::plan_uninstall(&dir)?,
        other => {
            return Err(CommandError::new(
                "unknown_kind",
                format!("未知的维护类型:{other}"),
            ));
        }
    };
    Ok(plan)
}

/// 确认并执行维护计划。执行前按当前磁盘状态重新规划(避免使用过期
/// 计划),返回完成动作数与重新部署指引。
#[tauri::command]
pub fn product_execute(kind: &str) -> Result<ExecuteResult, CommandError> {
    let (client, dir) = require_user_data_dir()?;
    let executed = match kind {
        "install" => {
            let package = RimePackage::bundled()?;
            let plan = manager::plan_install(&dir, &package)?;
            manager::execute(&plan, &dir, Some(&package))?
        }
        "uninstall" => {
            let plan = manager::plan_uninstall(&dir)?;
            manager::execute(&plan, &dir, None)?
        }
        other => {
            return Err(CommandError::new(
                "unknown_kind",
                format!("未知的维护类型:{other}"),
            ));
        }
    };
    Ok(ExecuteResult {
        done: executed,
        redeploy_guidance: client.redeploy_guidance().to_string(),
    })
}

/// 生成脱敏诊断报告(复制给 issue / 自查;不含学习词内容)。
#[tauri::command]
pub fn product_diagnostics() -> Result<String, CommandError> {
    let (client, dir) = require_user_data_dir()?;
    let package = RimePackage::bundled()?;
    let status = manager::install_status(&dir, client, Some(&package));
    let learning = manager::learning_summary(&dir);
    Ok(manager::diagnostics_report(
        &status,
        &package.version,
        &learning,
    ))
}

/// 导出学习数据快照(标准 Rime 文本格式;需要 rime_dict_manager)。
/// 返回快照文件路径。
#[tauri::command]
pub fn learning_export() -> Result<String, CommandError> {
    let (_client, dir) = require_user_data_dir()?;
    let snapshot = xhup_cli::learning::export(&dir, None, None).map_err(learning_error)?;
    Ok(snapshot.display().to_string())
}

/// 从快照恢复学习数据(跨安装迁移;需要 rime_dict_manager)。
#[tauri::command]
pub fn learning_import(snapshot: String) -> Result<(), CommandError> {
    let (_client, dir) = require_user_data_dir()?;
    xhup_cli::learning::import(&dir, std::path::Path::new(&snapshot), None).map_err(learning_error)
}

/// 重置学习数据(破坏性)。
///
/// 双重确认:`confirmed` 之外还要求 `dict_name` 逐字等于
/// `xhup_flow_user`(类型化确认,防止前端误传参直接销毁学习数据)。
#[tauri::command]
pub fn learning_reset(confirmed: bool, dict_name: String) -> Result<(), CommandError> {
    let (_client, dir) = require_user_data_dir()?;
    if dict_name != xhup_cli::learning::FLOW_USER_DICT_NAME {
        return Err(CommandError::new(
            "reset_not_confirmed",
            format!(
                "重置确认不匹配:期望 {},收到 {dict_name}",
                xhup_cli::learning::FLOW_USER_DICT_NAME
            ),
        ));
    }
    xhup_cli::learning::reset(&dir, confirmed).map_err(learning_error)
}
