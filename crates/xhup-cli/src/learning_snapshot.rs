//! Fail-closed boundary for native Rime userdb snapshots, not a LevelDB parser.
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{FLOW_USER_DICT_NAME, LearningError};

const MAX_BYTES: u64 = 64 * 1024 * 1024;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn invalid(reason: &str) -> LearningError {
    LearningError::InvalidSnapshot {
        reason: reason.to_owned(),
    }
}

fn validate(bytes: &[u8]) -> Result<(), LearningError> {
    let text = std::str::from_utf8(bytes).map_err(|_| invalid("snapshot must be UTF-8"))?;
    if text.contains('\0')
        || !text.starts_with("# Rime user dictionary\n")
            && !text.starts_with("# Rime user dictionary\r\n")
    {
        return Err(invalid(
            "missing standard Rime snapshot header or embedded NUL",
        ));
    }
    let mut metadata = BTreeMap::new();
    for line in text.lines() {
        // Rime TsvReader trims trailing whitespace before recognizing metadata.
        let line = line.trim_end();
        if line == "# no comment" {
            return Err(invalid("comment-mode switching is unsupported"));
        }
        if let Some(meta) = line.strip_prefix("#@") {
            let (key, value) = meta
                .split_once('\t')
                .ok_or_else(|| invalid("malformed metadata"))?;
            if value.is_empty() || value.contains('\t') || metadata.insert(key, value).is_some() {
                return Err(invalid("empty, malformed or duplicate metadata"));
            }
            if !matches!(
                key,
                "/db_name" | "/db_type" | "/rime_version" | "/tick" | "/user_id"
            ) {
                return Err(invalid("unsupported metadata key"));
            }
        }
    }
    let identity = metadata
        .get("/db_name")
        .copied()
        .ok_or_else(|| invalid("missing database identity"))?;
    // Historical native DB filenames normalize to the same stable dictionary.
    if ![
        FLOW_USER_DICT_NAME.to_owned(),
        format!("{FLOW_USER_DICT_NAME}.userdb"),
        format!("{FLOW_USER_DICT_NAME}.userdb.kct"),
        format!("{FLOW_USER_DICT_NAME}.userdb.txt"),
    ]
    .iter()
    .any(|allowed| allowed == identity)
    {
        return Err(invalid("snapshot does not belong to XHUP Flow"));
    }
    if metadata.get("/db_type") != Some(&"userdb") {
        return Err(invalid("missing or unsupported database type"));
    }
    let version = metadata
        .get("/rime_version")
        .ok_or_else(|| invalid("missing Rime version"))?;
    let parts: Vec<_> = version.split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
        || parts[0] != "1"
        || parts[1].parse::<u32>().map_or(true, |minor| minor > 16)
        || parts[2].parse::<u32>().is_err()
    {
        return Err(invalid(
            "unsupported Rime snapshot version (supported: 1.0.x through 1.16.x)",
        ));
    }
    if metadata
        .get("/tick")
        .is_some_and(|v| v.parse::<u64>().is_err())
    {
        return Err(invalid("invalid learning tick"));
    }
    Ok(())
}

/// Private staged copy: the manager never reopens the caller's mutable source.
/// Drop removes only this operation's exclusively-created directory.
pub(super) struct Snapshot {
    directory: PathBuf,
    pub(super) path: PathBuf,
}

impl Snapshot {
    pub(super) fn read(source: &Path) -> Result<Self, LearningError> {
        let io_error = |error: std::io::Error| LearningError::SnapshotIo { source: error };
        let file = File::open(source).map_err(io_error)?;
        if !file.metadata().map_err(io_error)?.is_file() {
            return Err(invalid("snapshot is not a regular file"));
        }
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(invalid("snapshot exceeds 64 MiB import limit"));
        }
        validate(&bytes)?;
        let base = fs::canonicalize(std::env::temp_dir()).map_err(io_error)?;
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        for _ in 0..128 {
            let directory = base.join(format!(
                "xhup-import-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            match builder.create(&directory) {
                Ok(()) => {
                    let snapshot = Self {
                        path: directory.join(super::snapshot_filename()),
                        directory,
                    };
                    let mut file = OpenOptions::new()
                        .create_new(true)
                        .write(true)
                        .open(&snapshot.path)
                        .map_err(io_error)?;
                    file.write_all(&bytes).map_err(io_error)?;
                    file.sync_all().map_err(io_error)?;
                    drop(file);
                    return Ok(snapshot);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_error(error)),
            }
        }
        Err(invalid("cannot reserve private import staging directory"))
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.directory);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(identity: &str) -> String {
        format!(
            "# Rime user dictionary\n#@/db_name\t{identity}\n#@/db_type\tuserdb\n#@/rime_version\t1.16.1\n#@/tick\t1\njb zq \t进去\tc=1 d=1 t=1\n"
        )
    }

    #[test]
    fn identity_and_historical_names() {
        for name in [
            "xhup_flow_user",
            "xhup_flow_user.userdb",
            "xhup_flow_user.userdb.kct",
            "xhup_flow_user.userdb.txt",
        ] {
            validate(fixture(name).as_bytes()).unwrap();
        }
        for name in [
            "foreign_audit_dictionary",
            "../xhup_flow_user",
            "xhup_flow_user.userdb/foreign",
            "xhup_flow_user.userdb.other",
        ] {
            assert!(validate(fixture(name).as_bytes()).is_err());
        }
    }

    #[test]
    fn malformed_absent_duplicate_and_unsupported_metadata() {
        let good = fixture(FLOW_USER_DICT_NAME);
        for text in [
            good.replace("#@/db_name\txhup_flow_user\n", ""),
            good.replace("#@/db_type\tuserdb", "#@/db_type\ttabledb"),
            good.replace("#@/db_name\t", "#@/db_name "),
            good.replace("1.16.1", "2.0.0"),
            good.replace("1.16.1", "1.17.0"),
            good.replace("1.16.1", "1.16"),
            good.replace("1.16.1", "1.16.1-beta"),
            good.replace("#@/rime_version\t1.16.1\n", ""),
            good.replace("#@/tick\t1", "#@/tick\tNaN"),
            format!("{good}#@/db_name\tforeign_audit_dictionary\n"),
            format!("{good}#@/unknown\tvalue\n"),
            format!("{good}# no comment\n"),
            format!("{good}\0"),
        ] {
            assert!(validate(text.as_bytes()).is_err());
        }
        validate(
            good.replace("1.16.1", "1.8.5")
                .replace('\n', "\r\n")
                .as_bytes(),
        )
        .unwrap();
        validate(good.replace("#@/tick\t1\n", "").as_bytes()).unwrap();
    }

    #[test]
    fn staging_is_private_independent_and_cleaned() {
        // Use the same exclusive staging allocator for this test's source fixture.
        // Fixture creation itself uses create_new, never truncates another file.
        let path =
            std::env::temp_dir().join(format!("xhup-snapshot-fixture-{}", std::process::id()));
        let mut source = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        source
            .write_all(fixture(FLOW_USER_DICT_NAME).as_bytes())
            .unwrap();
        drop(source);
        let snapshot = Snapshot::read(&path).unwrap();
        fs::write(&path, fixture("foreign_audit_dictionary")).unwrap();
        validate(&fs::read(&snapshot.path).unwrap()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&snapshot.directory)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        let dir = snapshot.directory.clone();
        drop(snapshot);
        assert!(!dir.exists());
        fs::remove_file(path).unwrap();
    }
}
