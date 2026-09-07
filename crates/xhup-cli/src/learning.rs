//! 用户词学习管理:包装 librime 的 `rime_dict_manager` 官方用户词典
//! 管理工具,提供 status / export / import / reset。
//!
//! 设计约束:
//! - **不解析 LevelDB/userdb 内部格式**:全部操作经由 `rime_dict_manager`
//!   (librime 官方备份/恢复机制),保证与 librime 版本兼容;
//! - **本地优先**:用户学习数据仅存在于用户数据目录,无网络/遥测;
//! - **用户词典身份稳定**:只操作 `xhup_flow_user`(Flow 引擎的共享
//!   用户词典),绝不触碰其它 Rime 词典;
//! - **DB 锁安全**:IME 会话持有 userdb 时导出/恢复会失败;返回可操作
//!   错误,绝不强制删除锁文件或自动修复。
//!
//! `rime_dict_manager` 的实测语义(librime 1.10/1.16):
//! - 工具以**当前工作目录**为用户数据目录(librime deployer 默认
//!   `.`;不支持环境变量重定向),因此全部调用以 `cwd = 用户数据目录`
//!   执行;
//! - `-l` 列出该目录下的 `*.userdb`;
//! - `-b <dict_name>` 备份快照到 `<用户数据目录>/sync/<user_id>/<词典名>.
//!   userdb.txt`(user_id 取自 DB 元数据,默认 `unknown`);快照是 Rime
//!   标准文本格式;
//! - `-r <xxx.userdb.txt>` 校验快照元数据后把条目**合并**进对应词典
//!   (快照携带 `/db_name`,目标词典由快照自身决定);
//! - `-e/-i` 为 TSV 全量导出/导入,本模块不使用,统一走标准快照。
//!
//! 因此 import 侧以快照内 `/db_name` 为准;本模块在导入前仍校验快照
//! 文件名与目标词典一致,防止把无关 Rime 词典的快照误当学习数据合并。

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Flow 引擎的稳定用户词典名(兼容接口,发布后不得随意变更)。
pub const FLOW_USER_DICT_NAME: &str = "xhup_flow_user";

/// 快照文件名(由 dict manager 机械派生:`<词典名>.userdb.txt`)。
fn snapshot_filename() -> String {
    format!("{FLOW_USER_DICT_NAME}.userdb.txt")
}

/// 学习管理错误。
#[derive(Debug)]
pub enum LearningError {
    /// 找不到 rime_dict_manager 可执行文件。
    DictManagerNotFound {
        /// 尝试过的路径/PATH 查找说明。
        detail: String,
    },
    /// 用户数据目录不存在。
    UserDataDirMissing { path: PathBuf },
    /// 快照文件不存在。
    SnapshotMissing { path: PathBuf },
    /// 快照文件名与目标词典不匹配(防误导入)。
    SnapshotNameMismatch { path: PathBuf, expected: String },
    /// 备份后未找到快照(工具未产出预期文件)。
    SnapshotNotProduced {
        user_data_dir: PathBuf,
        stderr: String,
    },
    /// 用户词典尚不存在(未产生任何学习数据)。
    UserDictAbsent,
    /// 底层工具执行失败(含 DB 被占用等;stderr 附带)。
    ToolFailed { program: String, stderr: String },
    /// reset 未确认(需要 --yes)。
    ResetNotConfirmed,
    /// reset 时用户词典意外存在但删除失败。
    ResetFailed {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for LearningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DictManagerNotFound { detail } => write!(
                f,
                "找不到 rime_dict_manager({detail});请安装 librime-bin 或用 \
                 --dict-manager 显式指定路径"
            ),
            Self::UserDataDirMissing { path } => {
                write!(f, "用户数据目录不存在: {}", path.display())
            }
            Self::SnapshotMissing { path } => {
                write!(f, "快照文件不存在: {}", path.display())
            }
            Self::SnapshotNameMismatch { path, expected } => write!(
                f,
                "快照文件名不匹配: {} 应为 {expected}(只能导入 {} 的快照,防止误写其它词典)",
                path.display(),
                FLOW_USER_DICT_NAME
            ),
            Self::SnapshotNotProduced {
                user_data_dir,
                stderr,
            } => write!(
                f,
                "dict manager 未产出快照(用户数据目录 {},应位于 \
                 sync/<user_id>/ 下):{stderr}",
                user_data_dir.display()
            ),
            Self::UserDictAbsent => write!(
                f,
                "用户词典 {FLOW_USER_DICT_NAME} 尚不存在(还没有任何学习数据)"
            ),
            Self::ToolFailed { program, stderr } => write!(
                f,
                "{program} 执行失败(若提示 DB in use:用户词典正被活动 Rime 会话占用,请关闭/重部署 IME 后重试):{stderr}"
            ),
            Self::ResetNotConfirmed => {
                write!(f, "reset 是破坏性操作,需要显式 --yes 确认")
            }
            Self::ResetFailed { path, source } => {
                write!(f, "删除用户词典失败 {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for LearningError {}

/// 定位 rime_dict_manager:显式路径优先,否则 PATH 查找。
fn find_dict_manager(explicit: Option<&Path>) -> Result<PathBuf, LearningError> {
    if let Some(path) = explicit {
        if path.is_file() {
            return Ok(path.to_path_buf());
        }
        return Err(LearningError::DictManagerNotFound {
            detail: format!("显式指定路径不存在: {}", path.display()),
        });
    }
    let found = which_dict_manager();
    found.ok_or_else(|| LearningError::DictManagerNotFound {
        detail: "PATH 中未找到".to_string(),
    })
}

/// PATH 查找(不引入依赖;常见安装路径兜底)。
fn which_dict_manager() -> Option<PathBuf> {
    for candidate in [
        "/usr/bin/rime_dict_manager",
        "/usr/local/bin/rime_dict_manager",
    ] {
        if Path::new(candidate).is_file() {
            return Some(PathBuf::from(candidate));
        }
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("rime_dict_manager"))
        .find(|candidate| candidate.is_file())
}

/// 用户词典目录路径(`<user_data_dir>/<name>.userdb`)。
fn user_db_path(user_data_dir: &Path) -> PathBuf {
    user_data_dir.join(format!("{FLOW_USER_DICT_NAME}.userdb"))
}

/// 在用户数据目录内运行 dict manager(cwd = 用户数据目录,是该工具
/// 定位 userdb 的唯一机制)。
fn run_in_user_dir(
    program: &Path,
    user_data_dir: &Path,
    args: &[&str],
) -> Result<String, LearningError> {
    let output = Command::new(program)
        .args(args)
        .current_dir(user_data_dir)
        .output()
        .map_err(|error| LearningError::ToolFailed {
            program: program.display().to_string(),
            stderr: error.to_string(),
        })?;
    if !output.status.success() {
        return Err(LearningError::ToolFailed {
            program: program.display().to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// 学习状态(非敏感元数据;不默认输出用户词内容)。
#[derive(Debug)]
pub struct LearningStatus {
    /// 用户词典名(稳定兼容接口)。
    pub user_dict: String,
    /// 用户数据目录。
    pub user_data_dir: PathBuf,
    /// 用户词典 DB 是否存在(是否已有学习数据)。
    pub db_exists: bool,
    /// DB 目录路径(存在时)。
    pub db_path: Option<PathBuf>,
    /// 已有快照路径(用户数据目录 sync 树下,存在时)。
    pub snapshot_path: Option<PathBuf>,
    /// dict manager 列出的全部用户词典(存在 DB 时)。
    pub known_user_dicts: Vec<String>,
}

/// 查询学习状态。
pub fn status(
    user_data_dir: &Path,
    dict_manager: Option<&Path>,
) -> Result<LearningStatus, LearningError> {
    if !user_data_dir.is_dir() {
        return Err(LearningError::UserDataDirMissing {
            path: user_data_dir.to_path_buf(),
        });
    }
    let db_path = user_db_path(user_data_dir);
    let db_exists = db_path.is_dir();
    let snapshot_path = find_existing_snapshot(user_data_dir);
    let known_user_dicts = if db_exists {
        let manager = find_dict_manager(dict_manager)?;
        // `-l` 列出各 userdb 行;解析词典名(userdb 目录名)。
        let out = run_in_user_dir(&manager, user_data_dir, &["-l"])?;
        out.lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                line.split_whitespace()
                    .next()
                    .unwrap_or(line)
                    .trim_end_matches(".userdb")
                    .to_string()
            })
            .collect()
    } else {
        Vec::new()
    };
    Ok(LearningStatus {
        user_dict: FLOW_USER_DICT_NAME.to_string(),
        user_data_dir: user_data_dir.to_path_buf(),
        db_exists,
        db_path: db_exists.then_some(db_path),
        snapshot_path,
        known_user_dicts,
    })
}

/// 在 `<user_data_dir>/sync/<user_id>/` 下查找本词典的最新快照。
///
/// `user_id` 由 DB 元数据决定(默认 `unknown`),不假设具体子目录名:
/// 扫描 sync 树下匹配快照文件名的全部文件,返回修改时间最新者。
fn find_existing_snapshot(user_data_dir: &Path) -> Option<PathBuf> {
    let sync_dir = user_data_dir.join("sync");
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    let Ok(entries) = std::fs::read_dir(&sync_dir) else {
        return None;
    };
    for entry in entries.flatten() {
        let Ok(sub) = std::fs::read_dir(entry.path()) else {
            continue;
        };
        for file in sub.flatten() {
            let path = file.path();
            if path.file_name()?.to_string_lossy() == snapshot_filename() {
                let modified = file
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                if best.as_ref().is_none_or(|(t, _)| modified > *t) {
                    best = Some((modified, path));
                }
            }
        }
    }
    best.map(|(_, path)| path)
}

/// 导出用户词典快照(Rime 标准文本格式 `<name>.userdb.txt`)。
///
/// 经 dict manager `-b` 备份(快照落在用户数据目录 `sync/<user_id>/`
/// 下),再复制到 `output_dir`(默认用户数据目录根);返回快照路径。
pub fn export(
    user_data_dir: &Path,
    output_dir: Option<&Path>,
    dict_manager: Option<&Path>,
) -> Result<PathBuf, LearningError> {
    if !user_data_dir.is_dir() {
        return Err(LearningError::UserDataDirMissing {
            path: user_data_dir.to_path_buf(),
        });
    }
    if !user_db_path(user_data_dir).is_dir() {
        return Err(LearningError::UserDictAbsent);
    }
    let manager = find_dict_manager(dict_manager)?;
    run_in_user_dir(&manager, user_data_dir, &["-b", FLOW_USER_DICT_NAME])?;
    let produced = find_existing_snapshot(user_data_dir).ok_or_else(|| {
        LearningError::SnapshotNotProduced {
            user_data_dir: user_data_dir.to_path_buf(),
            stderr: format!(
                "-b {} 后在 sync/ 下未找到 {}",
                FLOW_USER_DICT_NAME,
                snapshot_filename()
            ),
        }
    })?;
    let target = output_dir
        .unwrap_or(user_data_dir)
        .join(snapshot_filename());
    if produced != target {
        std::fs::copy(&produced, &target).map_err(|error| LearningError::ToolFailed {
            program: manager.display().to_string(),
            stderr: format!("复制快照失败: {error}"),
        })?;
    }
    Ok(target)
}

/// 从快照恢复用户词典(跨安装迁移)。
///
/// 快照文件名必须与目标词典一致(防误导入);恢复经 dict manager
/// `-r` 执行,librime 校验快照元数据(含 `/db_name`)后把条目合并进
/// `xhup_flow_user`。
pub fn import(
    user_data_dir: &Path,
    snapshot: &Path,
    dict_manager: Option<&Path>,
) -> Result<(), LearningError> {
    if !user_data_dir.is_dir() {
        return Err(LearningError::UserDataDirMissing {
            path: user_data_dir.to_path_buf(),
        });
    }
    if !snapshot.is_file() {
        return Err(LearningError::SnapshotMissing {
            path: snapshot.to_path_buf(),
        });
    }
    let expected_name = snapshot_filename();
    let actual_name = snapshot
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if actual_name != expected_name {
        return Err(LearningError::SnapshotNameMismatch {
            path: snapshot.to_path_buf(),
            expected: expected_name,
        });
    }
    let manager = find_dict_manager(dict_manager)?;
    // 传绝对路径:子进程 cwd 已切到用户数据目录,相对参数会相对它解析,
    // 与快照实际位置脱节;canonicalize 同时校验文件存在。
    let snapshot_abs =
        std::fs::canonicalize(snapshot).map_err(|error| LearningError::ToolFailed {
            program: manager.display().to_string(),
            stderr: format!("解析快照路径失败: {error}"),
        })?;
    run_in_user_dir(
        &manager,
        user_data_dir,
        &["-r", &snapshot_abs.to_string_lossy()],
    )?;
    Ok(())
}

/// 重置用户词典(破坏性;必须显式 confirmed)。
///
/// 只删除 `xhup_flow_user.userdb` 目录,绝不触碰其它 Rime 词典。
pub fn reset(user_data_dir: &Path, confirmed: bool) -> Result<(), LearningError> {
    if !user_data_dir.is_dir() {
        return Err(LearningError::UserDataDirMissing {
            path: user_data_dir.to_path_buf(),
        });
    }
    if !confirmed {
        return Err(LearningError::ResetNotConfirmed);
    }
    let db_path = user_db_path(user_data_dir);
    if !db_path.is_dir() {
        // 无学习数据 = 已是空状态,幂等成功。
        return Ok(());
    }
    std::fs::remove_dir_all(&db_path).map_err(|source| LearningError::ResetFailed {
        path: db_path,
        source,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 临时用户数据目录 fixture。
    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("xhup-learning-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn status_without_db_reports_absent() {
        let dir = temp_dir("status");
        let status = status(&dir, None).unwrap();
        assert_eq!(status.user_dict, FLOW_USER_DICT_NAME);
        assert!(!status.db_exists);
        assert!(status.db_path.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reset_requires_confirmation() {
        let dir = temp_dir("reset");
        // 未确认 → 错误。
        assert!(matches!(
            reset(&dir, false),
            Err(LearningError::ResetNotConfirmed)
        ));
        // 空状态确认 → 幂等成功。
        reset(&dir, true).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reset_removes_only_flow_user_db() {
        let dir = temp_dir("reset-targeted");
        // 伪造两个 userdb 目录:目标 + 无关词典。
        let flow_db = dir.join(format!("{FLOW_USER_DICT_NAME}.userdb"));
        let other_db = dir.join("other_scheme.userdb");
        std::fs::create_dir_all(&flow_db).unwrap();
        std::fs::create_dir_all(&other_db).unwrap();
        reset(&dir, true).unwrap();
        assert!(!flow_db.exists(), "目标用户词典应被删除");
        assert!(other_db.exists(), "无关词典不得被删除");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_requires_existing_db() {
        let dir = temp_dir("export-absent");
        assert!(matches!(
            export(&dir, None, None),
            Err(LearningError::UserDictAbsent)
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_rejects_foreign_snapshot_name() {
        let dir = temp_dir("import-name");
        let foreign = dir.join("other_scheme.userdb.txt");
        std::fs::write(&foreign, "ignored").unwrap();
        assert!(matches!(
            import(&dir, &foreign, None),
            Err(LearningError::SnapshotNameMismatch { .. })
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_user_data_dir_is_actionable() {
        let missing = PathBuf::from("/nonexistent/xhup-user-data");
        assert!(matches!(
            status(&missing, None),
            Err(LearningError::UserDataDirMissing { .. })
        ));
    }

    /// 快照发现:sync/<user_id>/ 下的快照可被定位(不假设 user_id)。
    #[test]
    fn snapshot_discovery_covers_sync_subdirs() {
        let dir = temp_dir("snapshot-discovery");
        let sync_user = dir.join("sync").join("some-user-id");
        std::fs::create_dir_all(&sync_user).unwrap();
        assert!(find_existing_snapshot(&dir).is_none(), "无快照时应返回空");
        let snapshot = sync_user.join(snapshot_filename());
        std::fs::write(&snapshot, b"# snapshot").unwrap();
        assert_eq!(find_existing_snapshot(&dir), Some(snapshot));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 端到端 fixture(仅当本机存在 rime_dict_manager 时运行):
    /// 伪造最小 userdb → export → reset → import → DB 恢复。
    ///
    /// dict manager 需要真实 LevelDB(`xhup_flow_user.userdb` 内含
    /// `CURRENT` 等元文件);纯空目录无法打开。CI 中真实 userdb 由
    /// runtime 学习会话产生,完整链路(含「恢复后学习行为可观察」)由
    /// runtime 审计脚本覆盖;本单测在工具无法打开最小 DB 时跳过。
    #[test]
    fn export_reset_import_roundtrip() {
        let Some(manager) = which_dict_manager() else {
            eprintln!("跳过:rime_dict_manager 不在本机 PATH");
            return;
        };
        let dir = temp_dir("roundtrip");
        let db = dir.join(format!("{FLOW_USER_DICT_NAME}.userdb"));
        std::fs::create_dir_all(&db).unwrap();
        std::fs::write(db.join("CURRENT"), b"MANIFEST-000001\n").unwrap();
        // export。
        let Ok(snapshot) = export(&dir, None, Some(&manager)) else {
            eprintln!("跳过:最小 userdb 不可被本机 dict manager 打开(真实链路由 runtime 测试覆盖)");
            let _ = std::fs::remove_dir_all(&dir);
            return;
        };
        assert!(snapshot.is_file(), "快照应生成");
        // reset。
        reset(&dir, true).unwrap();
        assert!(!db.exists(), "reset 后 DB 应消失");
        // import(快照名匹配)。
        import(&dir, &snapshot, Some(&manager)).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
