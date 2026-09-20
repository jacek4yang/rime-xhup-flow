//! 本地用户自适应状态的持久化(§16 / §25 第 9 步)。
//!
//! 把 [`UserModel`](xhup_analyzer::user_model::UserModel) 落盘为用户数据
//! 目录下的版本化 TSV 快照,要求与 [`learning`] 模块同源:
//!
//! - **versioned**:文件头携带 `xhup-user-model/v1` schema 与版本,
//!   加载侧拒绝未来版本并回退空模型(域逻辑在 `xhup-analyzer`,本模块
//!   只做文件编排);
//! - **atomic**:写盘 = 同目录临时文件 + rename(同卷原子);任何一步
//!   失败都不触碰既有快照;
//! - **corruption-safe**:加载任何失败(文件缺失除外)都回退**可用**的
//!   空模型并显式报告原因,绝不 panic、绝不静默;
//! - **reset**:破坏性删除需显式确认;
//! - **export/import**:TSV 即可移植快照;导入前严格校验,通过后原子替换;
//! - **隐私**:文件只存词形与计数(无句子/按键/时间戳);错误信息只含
//!   路径,绝不包含模型内容;
//! - **零遥测/离线**:只有本地文件系统操作。

use std::fs;
use std::path::{Path, PathBuf};

use xhup_analyzer::user_model::UserModel;
use xhup_analyzer::user_model::UserModelLoad;

/// 快照文件名(固定;不随版本变化,版本由文件头承载)。
pub const USER_MODEL_FILENAME: &str = "xhup_flow_user_model.tsv";

/// 临时文件后缀(同目录写入保证 rename 同卷原子)。
const TMP_SUFFIX: &str = ".tmp";

/// 快照的规范路径(`<user_data_dir>/xhup_flow_user_model.tsv`)。
pub fn path(user_data_dir: &Path) -> PathBuf {
    user_data_dir.join(USER_MODEL_FILENAME)
}

/// 用户状态错误。
#[derive(Debug)]
pub enum UserStateError {
    /// 用户数据目录不存在。
    UserDataDirMissing { path: PathBuf },
    /// 快照文件不存在(读取/重置语义下)。
    SnapshotMissing { path: PathBuf },
    /// 无法创建临时文件(写入阶段;既有快照未被触碰)。
    WriteTemporary {
        path: PathBuf,
        source: std::io::Error,
    },
    /// 临时文件无法替换为最终快照(替换阶段;旧快照已被 rename 消费,
    /// 中途失败可能留下临时文件,下次写入会覆盖)。
    ReplaceArtifact {
        temporary: PathBuf,
        artifact: PathBuf,
        source: std::io::Error,
    },
    /// 快照内容非法(导入路径;目标文件未被触碰)。
    InvalidSnapshot { reason: UserModelLoad },
    /// reset 未确认(需要显式确认)。
    ResetNotConfirmed,
    /// reset 时删除失败。
    ResetFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    /// 导出源不存在。
    ExportSourceMissing { path: PathBuf },
    /// 导出目标写入失败。
    ExportWriteFailed {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for UserStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserDataDirMissing { path } => {
                write!(f, "用户数据目录不存在: {}", path.display())
            }
            Self::SnapshotMissing { path } => {
                write!(f, "本地学习状态快照不存在: {}", path.display())
            }
            Self::WriteTemporary { path, source } => {
                write!(f, "无法写临时文件 {}: {source}", path.display())
            }
            Self::ReplaceArtifact {
                artifact, source, ..
            } => {
                write!(f, "无法替换快照 {}: {source}", artifact.display())
            }
            Self::InvalidSnapshot { reason } => {
                write!(f, "快照内容非法,已放弃导入: {reason}")
            }
            Self::ResetNotConfirmed => {
                write!(f, "reset 是破坏性操作,需要显式确认(--yes)")
            }
            Self::ResetFailed { path, source } => {
                write!(f, "删除快照失败 {}: {source}", path.display())
            }
            Self::ExportSourceMissing { path } => {
                write!(f, "无可导出的快照(尚无本地学习数据): {}", path.display())
            }
            Self::ExportWriteFailed { path, source } => {
                write!(f, "导出写入失败 {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for UserStateError {}

/// 从用户数据目录加载本地学习状态。
///
/// 快照不存在是**正常情况**(新装/未学习):返回空模型与 [`UserModelLoad::Ok`]。
/// 存在但损坏/版本过新:回退空模型并携带显式原因(调用方据此提示
/// 「本地学习数据不可用,已按出厂设置运行」)。
pub fn load(user_data_dir: &Path) -> Result<(UserModel, UserModelLoad), UserStateError> {
    require_dir(user_data_dir)?;
    let file = path(user_data_dir);
    if !file.is_file() {
        return Ok((UserModel::new(), UserModelLoad::Ok));
    }
    let text = fs::read_to_string(&file)
        .map_err(|source| UserStateError::WriteTemporary { path: file, source })?;
    Ok(UserModel::load_or_default(&text))
}

/// 把模型原子写入用户数据目录(临时文件 + 同卷 rename)。
pub fn save(user_data_dir: &Path, model: &UserModel) -> Result<PathBuf, UserStateError> {
    require_dir(user_data_dir)?;
    let file = path(user_data_dir);
    let tmp = file.with_file_name(format!(
        ".{}{TMP_SUFFIX}",
        file.file_name()
            .expect("快照文件名应有基名")
            .to_string_lossy()
    ));
    fs::write(&tmp, model.to_tsv()).map_err(|source| UserStateError::WriteTemporary {
        path: tmp.clone(),
        source,
    })?;
    fs::rename(&tmp, &file).map_err(|source| {
        let _ = fs::remove_file(&tmp);
        UserStateError::ReplaceArtifact {
            temporary: tmp,
            artifact: file.clone(),
            source,
        }
    })?;
    Ok(file)
}

/// 导出快照到指定目录(缺省写入用户数据目录)。
///
/// TSV 本身即可移植快照;导出是逐字节复制,不做格式转换。
pub fn export(user_data_dir: &Path, output_dir: Option<&Path>) -> Result<PathBuf, UserStateError> {
    require_dir(user_data_dir)?;
    let source = path(user_data_dir);
    if !source.is_file() {
        return Err(UserStateError::ExportSourceMissing { path: source });
    }
    let target_dir = output_dir.unwrap_or(user_data_dir);
    if !target_dir.is_dir() {
        return Err(UserStateError::UserDataDirMissing {
            path: target_dir.to_path_buf(),
        });
    }
    let target = target_dir.join(USER_MODEL_FILENAME);
    let contents = fs::read(&source).map_err(|io| UserStateError::ExportWriteFailed {
        path: source.clone(),
        source: io,
    })?;
    fs::write(&target, contents).map_err(|source| UserStateError::ExportWriteFailed {
        path: target.clone(),
        source,
    })?;
    Ok(target)
}

/// 从快照文件导入(跨安装迁移):严格校验后原子替换本地状态。
///
/// 校验失败(格式/版本/schema 不符)时本地既有状态**不被触碰**。
pub fn import(user_data_dir: &Path, snapshot: &Path) -> Result<PathBuf, UserStateError> {
    require_dir(user_data_dir)?;
    if !snapshot.is_file() {
        return Err(UserStateError::SnapshotMissing {
            path: snapshot.to_path_buf(),
        });
    }
    let text =
        fs::read_to_string(snapshot).map_err(|source| UserStateError::ExportWriteFailed {
            path: snapshot.to_path_buf(),
            source,
        })?;
    // 严格校验:任何结构问题都拒绝导入(与 load 的降级语义不同 ——
    // 导入是显式迁移动作,宁可拒绝也不静默丢弃目标状态)。
    let model =
        UserModel::from_tsv(&text).map_err(|reason| UserStateError::InvalidSnapshot { reason })?;
    save(user_data_dir, &model)
}

/// 重置本地学习状态(破坏性;只删除本模块拥有的快照文件,需确认)。
///
/// 与 [`learning::reset`] 的边界严格分离:这里只删 `xhup_flow_user_model.tsv`,
/// 绝不触碰 `xhup_flow_user.userdb`(librime 用户词典由 learning 管理)。
pub fn reset(user_data_dir: &Path, confirmed: bool) -> Result<(), UserStateError> {
    require_dir(user_data_dir)?;
    if !confirmed {
        return Err(UserStateError::ResetNotConfirmed);
    }
    let file = path(user_data_dir);
    if file.is_file() {
        fs::remove_file(&file)
            .map_err(|source| UserStateError::ResetFailed { path: file, source })?;
    }
    Ok(())
}

fn require_dir(user_data_dir: &Path) -> Result<(), UserStateError> {
    if !user_data_dir.is_dir() {
        return Err(UserStateError::UserDataDirMissing {
            path: user_data_dir.to_path_buf(),
        });
    }
    Ok(())
}

use std::fmt;

#[cfg(test)]
mod tests {
    use super::*;
    use xhup_analyzer::user_model::UserModelLoad;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xhup-user-state-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn model_with_entries() -> UserModel {
        let mut model = UserModel::new();
        model.observe("时间", 1);
        model.observe("时间", 2);
        model.observe("我们", 3);
        model
    }

    #[test]
    fn missing_snapshot_loads_as_empty_ok() {
        let dir = temp_dir("missing");
        let (model, status) = load(&dir).unwrap();
        assert!(model.is_empty());
        assert!(status.is_ok(), "快照缺失是正常情况,不是降级");
    }

    #[test]
    fn save_then_load_round_trips_and_persists_across_restart() {
        let dir = temp_dir("roundtrip");
        let written = save(&dir, &model_with_entries()).unwrap();
        assert_eq!(written, path(&dir));
        // 「重启」:全新加载路径,必须读到同样的状态。
        let (model, status) = load(&dir).unwrap();
        assert!(status.is_ok());
        assert_eq!(model.len(), 2);
        assert_eq!(model.to_tsv(), model_with_entries().to_tsv());
    }

    #[test]
    fn save_is_atomic_no_tmp_left_and_overwrites_cleanly() {
        let dir = temp_dir("atomic");
        save(&dir, &model_with_entries()).unwrap();
        let mut second = UserModel::new();
        second.observe("景点", 1);
        save(&dir, &second).unwrap();
        let (model, _) = load(&dir).unwrap();
        assert_eq!(model.len(), 1);
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x == TMP_SUFFIX.trim_start_matches('.'))
            })
            .collect();
        assert!(leftovers.is_empty(), "不得残留临时文件");
    }

    #[test]
    fn corrupt_snapshot_falls_back_to_empty_with_reason() {
        let dir = temp_dir("corrupt");
        fs::write(path(&dir), "not a valid tsv at all\n").unwrap();
        let (model, status) = load(&dir).unwrap();
        assert!(model.is_empty(), "损坏必须回退空模型");
        assert!(status.is_degraded());
    }

    #[test]
    fn future_version_snapshot_falls_back_with_reason() {
        let dir = temp_dir("future");
        fs::write(
            path(&dir),
            "# xhup-user-model/v1 version=99 words=0\nword\tselections\tlast_seq\n",
        )
        .unwrap();
        let (_, status) = load(&dir).unwrap();
        assert!(matches!(status, UserModelLoad::FutureVersion { found: 99 }));
    }

    #[test]
    fn import_rejects_invalid_snapshot_without_touching_local_state() {
        let dir = temp_dir("import-bad");
        save(&dir, &model_with_entries()).unwrap();
        let bad = dir.join("bad.tsv");
        fs::write(&bad, "garbage\n").unwrap();
        assert!(import(&dir, &bad).is_err());
        let (model, status) = load(&dir).unwrap();
        assert!(status.is_ok());
        assert_eq!(model.len(), 2, "导入失败不得触碰本地状态");
    }

    #[test]
    fn import_accepts_valid_snapshot_atomically() {
        let dir = temp_dir("import-ok");
        let incoming_dir = temp_dir("import-ok-src");
        save(&incoming_dir, &model_with_entries()).unwrap();
        let snapshot = export(&incoming_dir, None).unwrap();
        import(&dir, &snapshot).unwrap();
        let (model, status) = load(&dir).unwrap();
        assert!(status.is_ok());
        assert_eq!(model.len(), 2);
    }

    #[test]
    fn export_copies_bytes_exactly() {
        let dir = temp_dir("export");
        save(&dir, &model_with_entries()).unwrap();
        let out_dir = temp_dir("export-out");
        let target = export(&dir, Some(&out_dir)).unwrap();
        assert_eq!(
            fs::read(path(&dir)).unwrap(),
            fs::read(target).unwrap(),
            "导出必须逐字节一致(可移植快照)"
        );
    }

    #[test]
    fn reset_requires_confirmation_and_only_deletes_own_file() {
        let dir = temp_dir("reset");
        save(&dir, &model_with_entries()).unwrap();
        assert!(matches!(
            reset(&dir, false),
            Err(UserStateError::ResetNotConfirmed)
        ));
        reset(&dir, true).unwrap();
        assert!(!path(&dir).exists());
        // reset 是幂等的:文件已不存在时再次 reset 也成功。
        reset(&dir, true).unwrap();
    }

    #[test]
    fn errors_never_leak_user_words() {
        // 用 Display 渲染各类错误并断言不含任何词形(§5:错误只含路径)。
        let dir = temp_dir("privacy2");
        let mut model = UserModel::new();
        model.observe("绝密词形", 1);
        let saved = save(&dir, &model).unwrap();
        let rendered = format!("{}", UserStateError::ExportSourceMissing { path: saved });
        assert!(!rendered.contains("绝密词形"));
        let rendered = format!("{}", UserStateError::SnapshotMissing { path: path(&dir) });
        assert!(!rendered.contains("绝密词形"));
    }
}
