//! Tauri 命令层:控制中心 UI 与纯 Rust 管理核心(`manager`)之间的
//! 薄封装。
//!
//! 本模块不做业务决策:全部平台/安装/学习逻辑在 [`crate::manager`];
//! 这里只负责检测环境、调用核心、把结果序列化给前端。错误统一转为
//! 字符串(Tauri 命令的可序列化错误约定),文案与核心层一致。

use serde::Serialize;

use crate::manager::{
    self, InstallStatus, LearningSummary, ManagerError, Plan, RimeClient, RimePackage,
};

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

impl From<ManagerError> for String {
    fn from(error: ManagerError) -> Self {
        error.to_string()
    }
}

/// 当前环境的用户数据目录;目录不存在时返回可操作错误。
fn require_user_data_dir() -> Result<(RimeClient, std::path::PathBuf), String> {
    let (client, dir) = manager::detect_platform();
    let dir = dir.ok_or_else(|| "无法确定本平台的 Rime 用户数据目录".to_string())?;
    if !dir.is_dir() {
        return Err(format!(
            "Rime 用户数据目录不存在({}):请先安装对应的 Rime 客户端。",
            dir.display()
        ));
    }
    Ok((client, dir))
}

/// 读取控制中心完整产品状态。
#[tauri::command]
pub fn product_status() -> Result<ProductStatus, String> {
    let (client, dir) = manager::detect_platform();
    let rime_detected = dir.as_deref().is_some_and(std::path::Path::is_dir);
    let package = RimePackage::bundled();
    let install = dir
        .as_deref()
        .filter(|dir| dir.is_dir())
        .map(|dir| manager::install_status(dir, client));
    let update_available = match &install {
        Some(status) => match &status.installed_version {
            Some(installed) => *installed != package.version,
            None => false,
        },
        None => false,
    };
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
        learning,
    })
}

/// 产出维护计划(dry-run):`install` 覆盖安装/升级/修复(按当前状态
/// 自动决定 Write/Overwrite),`uninstall` 只列将删除的拥有文件。
/// 计划不落盘;确认后交 [`product_execute`]。
#[tauri::command]
pub fn product_plan(kind: &str) -> Result<Plan, String> {
    let (_client, dir) = require_user_data_dir()?;
    let plan = match kind {
        "install" => manager::plan_install(&dir, &RimePackage::bundled())?,
        "uninstall" => manager::plan_uninstall(&dir)?,
        other => return Err(format!("未知的维护类型:{other}")),
    };
    Ok(plan)
}

/// 确认并执行维护计划。执行前按当前磁盘状态重新规划(避免使用过期
/// 计划),返回完成动作数与重新部署指引。
#[tauri::command]
pub fn product_execute(kind: &str) -> Result<ExecuteResult, String> {
    let (client, dir) = require_user_data_dir()?;
    let executed = match kind {
        "install" => {
            let package = RimePackage::bundled();
            let plan = manager::plan_install(&dir, &package)?;
            manager::execute(&plan, &dir, Some(&package))?
        }
        "uninstall" => {
            let plan = manager::plan_uninstall(&dir)?;
            manager::execute(&plan, &dir, None)?
        }
        other => return Err(format!("未知的维护类型:{other}")),
    };
    Ok(ExecuteResult {
        done: executed,
        redeploy_guidance: client.redeploy_guidance().to_string(),
    })
}

/// 生成脱敏诊断报告(复制给 issue / 自查;不含学习词内容)。
#[tauri::command]
pub fn product_diagnostics() -> Result<String, String> {
    let (client, dir) = require_user_data_dir()?;
    let status = manager::install_status(&dir, client);
    let learning = manager::learning_summary(&dir);
    Ok(manager::diagnostics_report(
        &status,
        &RimePackage::bundled().version,
        &learning,
    ))
}

/// 导出学习数据快照(标准 Rime 文本格式;需要 rime_dict_manager)。
/// 返回快照文件路径。
#[tauri::command]
pub fn learning_export() -> Result<String, String> {
    let (_client, dir) = require_user_data_dir()?;
    let snapshot =
        xhup_cli::learning::export(&dir, None, None).map_err(|error| error.to_string())?;
    Ok(snapshot.display().to_string())
}

/// 从快照恢复学习数据(跨安装迁移;需要 rime_dict_manager)。
#[tauri::command]
pub fn learning_import(snapshot: String) -> Result<(), String> {
    let (_client, dir) = require_user_data_dir()?;
    xhup_cli::learning::import(&dir, std::path::Path::new(&snapshot), None)
        .map_err(|error| error.to_string())
}

/// 重置学习数据(破坏性;前端必须先取得用户明确确认)。
#[tauri::command]
pub fn learning_reset(confirmed: bool) -> Result<(), String> {
    let (_client, dir) = require_user_data_dir()?;
    xhup_cli::learning::reset(&dir, confirmed).map_err(|error| error.to_string())
}
