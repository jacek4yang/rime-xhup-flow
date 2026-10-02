//! Native ownership inspection is done on a locked PRIVATE copy, never a foreign original.
use super::{FLOW_USER_DICT_NAME, LearningError, snapshot};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
fn blocked(reason: &str) -> LearningError {
    LearningError::UnsafeOperation {
        reason: reason.into(),
    }
}
fn io_error(source: io::Error) -> LearningError {
    LearningError::SnapshotIo { source }
}

/// Stable inode; never unlink a lock that another process may already have open.
pub(super) fn operation_lock(root: &Path) -> Result<File, LearningError> {
    let path = root.join(".xhup-flow-learning.lock");
    if let Ok(meta) = fs::symlink_metadata(&path)
        && (!meta.is_file() || meta.file_type().is_symlink())
    {
        return Err(blocked("invalid operation lock path"));
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    let lock = options.open(path).map_err(io_error)?;
    lock.try_lock()
        .map_err(|_| blocked("another learning management operation is active"))?;
    Ok(lock)
}

fn native_lock(db: &Path) -> Result<File, LearningError> {
    let path = db.join("LOCK");
    let meta = fs::symlink_metadata(&path).map_err(io_error)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(blocked("invalid native database lock"));
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    let file = options.open(path).map_err(io_error)?;
    // POSIX LevelDB uses fcntl, NOT flock. Rust's File::try_lock uses flock there.
    #[cfg(unix)]
    rustix::fs::fcntl_lock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive)
        .map_err(|_| blocked("native userdb is active; stop the input method first"))?;
    // Windows File::try_lock and LevelDB both use overlapping LockFileEx ranges.
    #[cfg(windows)]
    file.try_lock()
        .map_err(|_| blocked("native userdb is active; stop the input method first"))?;
    #[cfg(not(any(unix, windows)))]
    return Err(blocked("native database locking unsupported"));
    Ok(file)
}

struct Workspace {
    root: PathBuf,
    cleanup: bool,
}
impl Workspace {
    fn new(root: &Path) -> Result<Self, LearningError> {
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        for _ in 0..128 {
            let path = root.join(format!(
                ".xhup-learning-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            match builder.create(&path) {
                Ok(()) => {
                    return Ok(Self {
                        root: path,
                        cleanup: true,
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(io_error(e)),
            }
        }
        Err(blocked("cannot reserve private management workspace"))
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        if self.cleanup {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

pub(super) struct NativeState {
    guard: Option<File>,
    workspace: Workspace,
    snapshot: snapshot::Snapshot,
    original: PathBuf,
}

fn leveldb_file(name: &str) -> bool {
    if matches!(name, "CURRENT" | "LOCK" | "LOG" | "LOG.old") {
        return true;
    }
    let number = if let Some(n) = name.strip_prefix("MANIFEST-") {
        n
    } else if let Some((n, ext)) = name.rsplit_once('.') {
        if !matches!(ext, "ldb" | "sst" | "log" | "dbtmp") {
            return false;
        }
        n
    } else {
        return false;
    };
    !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
}

impl NativeState {
    pub(super) fn inspect(root: &Path, manager: &Path) -> Result<Option<Self>, LearningError> {
        let original = super::user_db_path(root);
        match fs::symlink_metadata(&original) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io_error(e)),
            Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink() => {
                return Err(blocked(
                    "native userdb must be an owned directory, not a link or file",
                ));
            }
            Ok(_) => {}
        }
        let guard = native_lock(&original)?;
        let workspace = Workspace::new(root)?;
        let copied = workspace.root.join(format!("{FLOW_USER_DICT_NAME}.userdb"));
        fs::create_dir(&copied).map_err(io_error)?;
        let mut size = 0u64;
        for (i, entry) in fs::read_dir(&original).map_err(io_error)?.enumerate() {
            let entry = entry.map_err(io_error)?;
            let meta = fs::symlink_metadata(entry.path()).map_err(io_error)?;
            if i >= 4096 || !meta.is_file() || !leveldb_file(&entry.file_name().to_string_lossy()) {
                return Err(blocked("ambiguous or excessive native database contents"));
            }
            size = size.saturating_add(meta.len());
            if size > 256 * 1024 * 1024 {
                return Err(blocked("native management copy exceeds 256 MiB"));
            }
            // Reopening then closing original LOCK would release POSIX process locks!
            if entry.file_name() != "LOCK" {
                fs::copy(entry.path(), copied.join(entry.file_name())).map_err(io_error)?;
            }
        }
        if !copied.join("CURRENT").is_file() {
            return Err(blocked("native database lacks CURRENT"));
        }
        super::run_in_user_dir(manager, &workspace.root, &["-b", FLOW_USER_DICT_NAME])?;
        let output = super::find_existing_snapshot(&workspace.root)
            .ok_or_else(|| blocked("private native inspection produced no snapshot"))?;
        let snapshot = snapshot::Snapshot::read(&output)?;
        Ok(Some(Self {
            guard: Some(guard),
            workspace,
            snapshot,
            original,
        }))
    }

    pub(super) fn export_to(&self, destination: &Path) -> Result<PathBuf, LearningError> {
        fs::create_dir_all(destination).map_err(io_error)?;
        let destination = fs::canonicalize(destination).map_err(io_error)?;
        let output = destination.join(super::snapshot_filename());
        if let Ok(meta) = output.symlink_metadata() {
            if meta.file_type().is_symlink() {
                return Err(blocked("export destination is a symbolic link"));
            }
            let _validated = snapshot::Snapshot::read(&output)?;
        }
        let staging = Workspace::new(&destination)?;
        let staged = staging.root.join(super::snapshot_filename());
        fs::copy(&self.snapshot.path, &staged).map_err(io_error)?;
        File::open(&staged)
            .map_err(io_error)?
            .sync_all()
            .map_err(io_error)?;
        fs::rename(staged, &output).map_err(io_error)?;
        Ok(output)
    }

    pub(super) fn reset(mut self) -> Result<(), LearningError> {
        let retired = self.workspace.root.join("retired.userdb");
        // Rename while LOCK is held; never delete a newly-created original path.
        fs::rename(&self.original, &retired).map_err(io_error)?;
        self.guard.take();
        if let Err(source) = fs::remove_dir_all(&retired) {
            self.workspace.cleanup = false;
            return Err(LearningError::ResetFailed {
                path: retired,
                source,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn operation_lock_serializes_and_releases_without_unlinking() {
        let workspace = Workspace::new(&std::env::temp_dir()).unwrap();
        let first = operation_lock(&workspace.root).unwrap();
        assert!(operation_lock(&workspace.root).is_err());
        drop(first);
        assert!(operation_lock(&workspace.root).is_ok());
        assert!(workspace.root.join(".xhup-flow-learning.lock").is_file());
    }
    #[test]
    fn only_recognized_native_files_are_owned() {
        for name in [
            "CURRENT",
            "LOCK",
            "MANIFEST-000001",
            "000010.ldb",
            "000011.log",
        ] {
            assert!(leveldb_file(name));
        }
        for name in ["personal.txt", "../CURRENT", "MANIFEST-", ".ldb", "notes"] {
            assert!(!leveldb_file(name));
        }
    }
}
