//! XHUP Flow 安装诊断与运行时合同检查 (doctor)。
//!
//! 检查项:
//! 1. 用户数据目录存在性
//! 2. 方案文件存在性与格式 (xhup_flow.schema.yaml, xhup_flow_static.schema.yaml)
//! 3. 核心词典文件完整性
//! 4. Lua 运行时模块完整性 (annotation.lua, quick_hint.lua, init.lua, data/quick_hints.lua)
//! 5. 平台 librime-lua 支持探测与 2.0 mandatory Lua 合同判定

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

/// XHUP Flow 核心 Rime 文件清单 (不含 Lua)。
pub const CORE_RIME_FILES: &[&str] = &[
    "xhup_flow.schema.yaml",
    "xhup_flow.dict.yaml",
    "xhup_flow_static.schema.yaml",
    "xhup_flow_chars.dict.yaml",
    "xhup_flow_words.dict.yaml",
    "xhup_flow_shortcuts.dict.yaml",
    "xhup_flow_word_shortcuts.dict.yaml",
    "xhup_flow_fixed_first_shortcuts.dict.yaml",
    "xhup_flow_flow.schema.yaml",
    "xhup_flow_flow.dict.yaml",
    "xhup_flow_learn.schema.yaml",
    "xhup_flow_learn.dict.yaml",
];

/// XHUP Flow Lua 运行时模块清单。
pub const LUA_RUNTIME_FILES: &[&str] = &[
    "lua/xhup_flow/annotation.lua",
    "lua/xhup_flow/quick_hint.lua",
    "lua/xhup_flow/init.lua",
    "lua/xhup_flow/data/quick_hints.lua",
];

/// 诊断过程错误。
#[derive(Debug)]
pub enum DoctorError {
    /// 用户数据目录不存在
    UserDataDirNotFound(PathBuf),
    /// 合同检查未通过
    ContractFailed(String),
}

impl fmt::Display for DoctorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserDataDirNotFound(path) => {
                write!(f, "Rime 用户数据目录不存在: {}", path.display())
            }
            Self::ContractFailed(reason) => {
                write!(f, "XHUP Flow 诊断发现不满足合同项: {reason}")
            }
        }
    }
}

impl Error for DoctorError {}

/// 诊断结果报告。
#[derive(Clone, Debug)]
pub struct DoctorReport {
    pub user_data_dir: PathBuf,
    pub installed_schemas: Vec<String>,
    pub missing_core_files: Vec<String>,
    pub missing_lua_files: Vec<String>,
    pub lua_contract_ok: bool,
    pub messages: Vec<String>,
}

impl DoctorReport {
    /// 格式化为输出报告字符串。
    pub fn format_report(&self) -> String {
        let mut out = String::new();
        out.push_str("========================================\n");
        out.push_str("   XHUP Flow 2.0 运行环境诊断报告 (Doctor)\n");
        out.push_str("========================================\n");
        out.push_str(&format!("用户数据目录: {}\n", self.user_data_dir.display()));
        out.push_str(&format!(
            "已发现方案: {}\n",
            if self.installed_schemas.is_empty() {
                "(未安装任何 XHUP 方案)".to_string()
            } else {
                self.installed_schemas.join(", ")
            }
        ));

        if self.missing_core_files.is_empty() {
            out.push_str("核心方案与词典: 完整 (12/12)\n");
        } else {
            out.push_str(&format!(
                "核心方案与词典: 缺失 {} 个文件 ({})\n",
                self.missing_core_files.len(),
                self.missing_core_files.join(", ")
            ));
        }

        if self.missing_lua_files.is_empty() {
            out.push_str("Lua 运行时模块: 完整 (4/4)\n");
        } else {
            out.push_str(&format!(
                "Lua 运行时模块: 缺失 {} 个模块 ({})\n",
                self.missing_lua_files.len(),
                self.missing_lua_files.join(", ")
            ));
        }

        out.push_str(&format!(
            "2.0 Mandatory Lua 合同: {}\n",
            if self.lua_contract_ok {
                "满足 (PASS)"
            } else {
                "未满足 (FAIL)"
            }
        ));

        if !self.messages.is_empty() {
            out.push_str("\n诊断与处理建议:\n");
            for msg in &self.messages {
                out.push_str(&format!("  * {msg}\n"));
            }
        }
        out.push_str("========================================\n");
        out
    }
}

/// 探测系统/平台 librime-lua 插件可用性。
pub fn probe_system_lua_plugin() -> Result<&'static str, &'static str> {
    probe_system_lua_plugin_with(None, &|p| p.is_file())
}

/// 探测系统/平台 librime-lua 插件可用性 (带探测函数注入)。
pub fn probe_system_lua_plugin_with<F>(
    plugins_dir: Option<&Path>,
    file_exists: &F,
) -> Result<&'static str, &'static str>
where
    F: Fn(&Path) -> bool,
{
    if let Some(dir) = plugins_dir {
        if file_exists(&dir.join("librime-lua.so"))
            || file_exists(&dir.join("librime-plugin-lua.so"))
        {
            return Ok("自定义插件目录内检测到 librime-lua 插件");
        }
        return Err("自定义插件目录内未找到 librime-lua.so 或 librime-plugin-lua.so");
    }

    if cfg!(target_os = "windows") {
        return Ok("Windows 小狼毫 (Weasel ≥ 0.15 内置 librime-lua)");
    }
    if cfg!(target_os = "macos") {
        return Ok("macOS 鼠须管 (Squirrel ≥ 1.0 内置 librime-lua)");
    }

    let candidates = [
        "/usr/lib/rime-plugins/librime-lua.so",
        "/usr/lib/rime-plugins/librime-plugin-lua.so",
        "/usr/lib/x86_64-linux-gnu/rime-plugins/librime-lua.so",
        "/usr/lib/x86_64-linux-gnu/rime-plugins/librime-plugin-lua.so",
        "/usr/lib/aarch64-linux-gnu/rime-plugins/librime-lua.so",
        "/usr/lib/aarch64-linux-gnu/rime-plugins/librime-plugin-lua.so",
        "/usr/lib64/rime-plugins/librime-lua.so",
        "/usr/lib64/rime-plugins/librime-plugin-lua.so",
        "/usr/local/lib/rime-plugins/librime-lua.so",
        "/usr/local/lib/rime-plugins/librime-plugin-lua.so",
    ];
    for path in &candidates {
        if file_exists(Path::new(path)) {
            return Ok("Linux 系统 librime-lua 插件在场");
        }
    }
    if let Some(dir) = std::env::var_os("RIME_PLUGINS_DIR") {
        let p = PathBuf::from(dir);
        if file_exists(&p.join("librime-lua.so")) || file_exists(&p.join("librime-plugin-lua.so")) {
            return Ok("RIME_PLUGINS_DIR 内检测到 librime-lua 插件");
        }
    }
    Err(
        "未检测到 librime-plugin-lua 插件。Debian/Ubuntu: sudo apt install librime-plugin-lua; 或改用纯静态方案 xhup_flow_static",
    )
}

/// 执行运行环境诊断与合同自检。
pub fn inspect_installation(
    user_data_dir: &Path,
    schema_filter: Option<&str>,
    plugins_dir: Option<&Path>,
) -> Result<DoctorReport, DoctorError> {
    inspect_installation_with(user_data_dir, schema_filter, plugins_dir, &|p| p.is_file())
}

/// 执行运行环境诊断与合同自检 (带文件探测器注入)。
pub fn inspect_installation_with<F>(
    user_data_dir: &Path,
    schema_filter: Option<&str>,
    plugins_dir: Option<&Path>,
    file_exists: &F,
) -> Result<DoctorReport, DoctorError>
where
    F: Fn(&Path) -> bool,
{
    if !user_data_dir.exists() {
        return Err(DoctorError::UserDataDirNotFound(
            user_data_dir.to_path_buf(),
        ));
    }

    let mut installed_schemas = Vec::new();
    if user_data_dir.join("xhup_flow.schema.yaml").is_file() {
        installed_schemas.push("xhup_flow".to_string());
    }
    if user_data_dir.join("xhup_flow_static.schema.yaml").is_file() {
        installed_schemas.push("xhup_flow_static".to_string());
    }

    let mut missing_core_files = Vec::new();
    for file in CORE_RIME_FILES {
        if !user_data_dir.join(file).is_file() {
            missing_core_files.push((*file).to_string());
        }
    }

    let mut missing_lua_files = Vec::new();
    for file in LUA_RUNTIME_FILES {
        if !user_data_dir.join(file).is_file() {
            missing_lua_files.push((*file).to_string());
        }
    }

    let target_schema = schema_filter.unwrap_or_else(|| {
        if installed_schemas.contains(&"xhup_flow".to_string()) {
            "xhup_flow"
        } else if installed_schemas.contains(&"xhup_flow_static".to_string()) {
            "xhup_flow_static"
        } else {
            "xhup_flow"
        }
    });

    let mut messages = Vec::new();
    let mut lua_contract_ok = true;

    if target_schema == "xhup_flow" {
        // 主方案 mandatory Lua 合同
        if !missing_lua_files.is_empty() {
            lua_contract_ok = false;
            messages.push(format!(
                "主方案 xhup_flow 要求完整 Lua 运行时，但缺少模块: {}",
                missing_lua_files.join(", ")
            ));
        }

        match probe_system_lua_plugin_with(plugins_dir, file_exists) {
            Ok(info) => {
                messages.push(format!("平台 Lua 运行时: {}", info));
            }
            Err(guidance) => {
                lua_contract_ok = false;
                messages.push(format!("平台缺少 Lua 支持: {}", guidance));
            }
        }

        if lua_contract_ok {
            messages.push("xhup_flow 2.0 mandatory Lua 合同已满足。".to_string());
        } else {
            messages.push(
                "若无法在当前环境安装 librime-lua，请切换使用纯静态方案 xhup_flow_static。"
                    .to_string(),
            );
        }
    } else if target_schema == "xhup_flow_static" {
        messages.push(
            "方案 xhup_flow_static 处于纯静态零-Lua 模式，与 v1.0.0 冻结肌肉记忆完全一致。"
                .to_string(),
        );
        messages.push("静态方案不需要 librime-lua 运行时支持。".to_string());
    } else {
        lua_contract_ok = false;
        messages.push(format!("未知目标方案: {target_schema}"));
    }

    Ok(DoctorReport {
        user_data_dir: user_data_dir.to_path_buf(),
        installed_schemas,
        missing_core_files,
        missing_lua_files,
        lua_contract_ok,
        messages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xhup-doctor-test-{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn inspect_healthy_installation() {
        let dir = create_test_dir("healthy");
        for file in CORE_RIME_FILES {
            fs::write(dir.join(file), "# schema or dict").unwrap();
        }
        for file in LUA_RUNTIME_FILES {
            let target = dir.join(file);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, "-- lua").unwrap();
        }

        let report = inspect_installation_with(&dir, Some("xhup_flow"), None, &|_| true).unwrap();
        assert!(report.lua_contract_ok);
        assert!(report.missing_core_files.is_empty());
        assert!(report.missing_lua_files.is_empty());
        let text = report.format_report();
        assert!(text.contains("满足 (PASS)"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn inspect_missing_lua_modules() {
        let dir = create_test_dir("missing-lua");
        for file in CORE_RIME_FILES {
            fs::write(dir.join(file), "# schema").unwrap();
        }
        let report = inspect_installation_with(&dir, Some("xhup_flow"), None, &|_| true).unwrap();
        assert!(!report.lua_contract_ok);
        assert_eq!(report.missing_lua_files.len(), 4);
        let text = report.format_report();
        assert!(text.contains("未满足 (FAIL)"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn inspect_static_schema_fallback() {
        let dir = create_test_dir("static");
        fs::write(dir.join("xhup_flow_static.schema.yaml"), "# static").unwrap();
        let report =
            inspect_installation_with(&dir, Some("xhup_flow_static"), None, &|_| false).unwrap();
        assert!(report.lua_contract_ok);
        let text = report.format_report();
        assert!(text.contains("纯静态零-Lua 模式"));

        let _ = fs::remove_dir_all(&dir);
    }
}
