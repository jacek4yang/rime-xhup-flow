//! Reversible ownership of Rime's shared default.custom.yaml.
//! The caller holds the installation lock. Original bytes are snapshotted once,
//! never replaced on upgrade. Foreign edits and symlinks fail closed.
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) const DEFAULT: &str = "default.custom.yaml";
const BACKUP: &str = ".xhup-flow-default-backup.json";
pub(crate) const CONTENTS: &str = include_str!("../../../rime/package/default.custom.yaml");
const MAX_CONFIG: u64 = 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    version: u8,
    original: Option<Vec<u8>>,
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn read_regular(path: &Path, limit: u64) -> io::Result<Option<Vec<u8>>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > limit {
        return Err(invalid(
            "exclusive Rime configuration/backup must be a bounded ordinary file",
        ));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(invalid("exclusive Rime configuration exceeds size bound"));
    }
    Ok(Some(bytes))
}
fn snapshot(root: &Path) -> io::Result<Option<Snapshot>> {
    let Some(bytes) = read_regular(&root.join(BACKUP), MAX_CONFIG * 4 + 128)? else {
        return Ok(None);
    };
    let value: Snapshot = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("invalid exclusive Rime backup; original file retained"))?;
    if value.version != 1
        || value
            .original
            .as_ref()
            .is_some_and(|v| v.len() as u64 > MAX_CONFIG)
    {
        return Err(invalid("unsupported or oversized exclusive Rime backup"));
    }
    Ok(Some(value))
}
pub(crate) fn active_or_backed_up(root: &Path) -> bool {
    has_backup(root)
        || read_regular(&root.join(DEFAULT), MAX_CONFIG)
            .ok()
            .flatten()
            .as_deref()
            == Some(CONTENTS.as_bytes())
}

pub(crate) fn has_backup(root: &Path) -> bool {
    match fs::symlink_metadata(root.join(BACKUP)) {
        Ok(_) => true,
        Err(error) => error.kind() != io::ErrorKind::NotFound,
    }
}
pub(crate) fn validate_install(root: &Path) -> io::Result<()> {
    let current = read_regular(&root.join(DEFAULT), MAX_CONFIG)?;
    if let Some(saved) = snapshot(root)? {
        if current.as_deref() != Some(CONTENTS.as_bytes()) && current != saved.original {
            return Err(invalid(
                "default.custom.yaml changed outside XHUP Flow; back it up and resolve the conflict before installation",
            ));
        }
    } else if current.as_deref() == Some(CONTENTS.as_bytes()) {
        return Err(invalid(
            "exclusive configuration exists without its original backup; restore the original configuration before managed installation",
        ));
    }
    Ok(())
}
pub(crate) fn ensure_backup(root: &Path) -> io::Result<()> {
    validate_install(root)?;
    if has_backup(root) {
        return Ok(());
    }
    let saved = Snapshot {
        version: 1,
        original: read_regular(&root.join(DEFAULT), MAX_CONFIG)?,
    };
    let bytes = serde_json::to_vec(&saved).map_err(io::Error::other)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let path = root.join(BACKUP);
    let mut file = options.open(&path)?;
    if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
        drop(file);
        // Only the exclusively created file belongs to this failed operation.
        return match fs::remove_file(&path) {
            Ok(()) => Err(error),
            Err(cleanup) => Err(io::Error::other(format!(
                "{error}; backup cleanup failed: {cleanup}"
            ))),
        };
    }
    Ok(())
}
pub(crate) fn validate_restore(root: &Path) -> io::Result<()> {
    let saved = snapshot(root)?.ok_or_else(|| {
        invalid("no exclusive configuration backup; refusing to remove a shared Rime file")
    })?;
    let current = read_regular(&root.join(DEFAULT), MAX_CONFIG)?;
    if current.as_deref() != Some(CONTENTS.as_bytes()) && current != saved.original {
        return Err(invalid(
            "default.custom.yaml was edited after installation; refusing to overwrite user changes",
        ));
    }
    Ok(())
}
pub(crate) fn restore(root: &Path) -> io::Result<()> {
    validate_restore(root)?;
    let saved = snapshot(root)?.ok_or_else(|| invalid("exclusive backup disappeared"))?;
    let destination = root.join(DEFAULT);
    if read_regular(&destination, MAX_CONFIG)? != saved.original {
        if let Some(bytes) = saved.original {
            let temp = root.join(format!(
                ".xhup-flow-default-restore-{}-{}.tmp",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temp)?;
            let result = file.write_all(&bytes).and_then(|()| file.sync_all());
            drop(file);
            let result = result.and_then(|()| fs::rename(&temp, &destination));
            if let Err(error) = result {
                if let Err(cleanup) = fs::remove_file(&temp) {
                    return Err(io::Error::other(format!(
                        "{error}; restore cleanup failed: {cleanup}"
                    )));
                }
                return Err(error);
            }
        } else {
            fs::remove_file(&destination)?;
        }
    }
    fs::remove_file(root.join(BACKUP))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(std::path::PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "xhup-exclusive-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn upgrades_preserve_first_original_and_restore_exact_non_utf8_bytes() {
        let root = Temp::new();
        let original = b"# custom\r\npatch:\r\n  schema_list: [other]\r\n\xff";
        fs::write(root.0.join(DEFAULT), original).unwrap();
        ensure_backup(&root.0).unwrap();
        let first = fs::read(root.0.join(BACKUP)).unwrap();
        fs::write(root.0.join(DEFAULT), CONTENTS).unwrap();
        ensure_backup(&root.0).unwrap();
        assert_eq!(fs::read(root.0.join(BACKUP)).unwrap(), first);
        restore(&root.0).unwrap();
        assert_eq!(fs::read(root.0.join(DEFAULT)).unwrap(), original);
        assert!(!has_backup(&root.0));
    }
    #[test]
    fn fresh_install_restores_absence_and_refuses_unowned_default() {
        let root = Temp::new();
        ensure_backup(&root.0).unwrap();
        fs::write(root.0.join(DEFAULT), CONTENTS).unwrap();
        restore(&root.0).unwrap();
        assert!(!root.0.join(DEFAULT).exists());
        fs::write(root.0.join(DEFAULT), CONTENTS).unwrap();
        assert!(ensure_backup(&root.0).is_err());
        assert!(restore(&root.0).is_err());
    }
    #[test]
    fn outside_edits_and_corrupt_backups_are_not_overwritten() {
        let root = Temp::new();
        ensure_backup(&root.0).unwrap();
        fs::write(root.0.join(DEFAULT), b"foreign edit").unwrap();
        assert!(ensure_backup(&root.0).is_err());
        assert!(restore(&root.0).is_err());
        assert_eq!(fs::read(root.0.join(DEFAULT)).unwrap(), b"foreign edit");
        fs::write(root.0.join(BACKUP), b"{}").unwrap();
        assert!(restore(&root.0).is_err());
    }
    #[test]
    fn failed_install_or_interrupted_restore_can_retry_without_losing_original() {
        let root = Temp::new();
        fs::write(root.0.join(DEFAULT), b"original").unwrap();
        ensure_backup(&root.0).unwrap();
        ensure_backup(&root.0).unwrap();
        restore(&root.0).unwrap();
        assert_eq!(fs::read(root.0.join(DEFAULT)).unwrap(), b"original");
    }
    #[cfg(unix)]
    #[test]
    fn symlinks_are_refused_without_touching_targets() {
        use std::os::unix::fs::symlink;
        let root = Temp::new();
        let foreign = root.0.join("foreign");
        fs::write(&foreign, b"safe").unwrap();
        symlink(&foreign, root.0.join(DEFAULT)).unwrap();
        assert!(ensure_backup(&root.0).is_err());
        fs::remove_file(root.0.join(DEFAULT)).unwrap();
        symlink(&foreign, root.0.join(BACKUP)).unwrap();
        assert!(ensure_backup(&root.0).is_err());
        assert!(restore(&root.0).is_err());
        assert_eq!(fs::read(&foreign).unwrap(), b"safe");
    }
}
