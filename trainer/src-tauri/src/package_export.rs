//! Export immutable package bytes, never live Rime/user files.
//! A new private directory is exclusively reserved; existing output is refused.
//! Errors clean up only that new tree. This is not a power-loss transaction: an
//! interrupted process may leave an incomplete tree, which a retry will refuse.
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::manager::{ManagerError, OWNED_FILES, RimePackage};

const DOCUMENTS: &[(&str, &[u8])] = &[
    (
        "INSTALL.md",
        include_bytes!("../../../rime/package/INSTALL.md"),
    ),
    (
        "NOTICE.md",
        include_bytes!("../../../rime/package/NOTICE.md"),
    ),
    (
        "licenses/project-LGPL-3.0.txt",
        include_bytes!("../../../LICENSE"),
    ),
    (
        "licenses/GPL-3.0.txt",
        include_bytes!("../../../LICENSE.GPL-3.0"),
    ),
    (
        "licenses/wanxiang-CC-BY-4.0.txt",
        include_bytes!("../../../data/words/LICENSE.wanxiang"),
    ),
    (
        "licenses/pinyin-data-MIT.txt",
        include_bytes!("../../../data/hanzi/LICENSE.pinyin-data"),
    ),
    (
        "licenses/kdconv-Apache-2.0.txt",
        include_bytes!("../../../data/corpus/LICENSE.kdconv"),
    ),
    (
        "licenses/ptt-Apache-2.0.txt",
        include_bytes!("../../../data/corpus/LICENSE.ptt"),
    ),
];

fn invalid(message: &str) -> ManagerError {
    ManagerError::PackageInvalid {
        missing: message.into(),
    }
}

fn io_error(path: &Path, source: io::Error) -> ManagerError {
    ManagerError::Io {
        path: path.to_owned(),
        source,
    }
}

pub fn export(destination: &Path, package: &RimePackage) -> Result<PathBuf, ManagerError> {
    export_with(destination, package, |path, bytes| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(bytes)?;
        file.sync_all()
    })
}

fn export_with(
    destination: &Path,
    package: &RimePackage,
    mut write: impl FnMut(&Path, &[u8]) -> io::Result<()>,
) -> Result<PathBuf, ManagerError> {
    let names: BTreeSet<_> = package
        .files
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    if package.files.len() != OWNED_FILES.len()
        || names != OWNED_FILES.iter().copied().collect()
        || package.files.iter().any(|(_, text)| text.is_empty())
        || package.version.len() > 64
        || !package
            .version
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_digit)
        || !package
            .version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-+".contains(&b))
    {
        return Err(invalid("export inventory/version"));
    }
    let metadata =
        fs::symlink_metadata(destination).map_err(|error| io_error(destination, error))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(invalid(
            "export destination must be an existing ordinary directory",
        ));
    }
    // Resolve parent aliases once. No caller-provided relative payload paths:
    // the entire inventory is the fixed ownership list above.
    let destination =
        fs::canonicalize(destination).map_err(|error| io_error(destination, error))?;
    let target = destination.join(format!("xhup-flow-rime-v{}", package.version));
    let builder = fs::DirBuilder::new();
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt as _;
        let mut builder = builder;
        builder.mode(0o700);
        builder
    };
    // Never truncate existing exports, including an empty directory or symlink.
    builder
        .create(&target)
        .map_err(|error| io_error(&target, error))?;
    let result = (|| {
        for (name, bytes) in package
            .files
            .iter()
            .map(|(n, s)| (n.as_str(), s.as_bytes()))
            .chain(DOCUMENTS.iter().copied())
        {
            let path = target.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
            }
            write(&path, bytes).map_err(|error| io_error(&path, error))?;
        }
        Ok::<_, ManagerError>(())
    })();
    if let Err(error) = result {
        if let Err(cleanup) = fs::remove_dir_all(&target) {
            return Err(io_error(
                &target,
                io::Error::other(format!(
                    "{error}; incomplete export cleanup failed: {cleanup}"
                )),
            ));
        }
        return Err(error);
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "xhup-export-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn package() -> RimePackage {
        RimePackage {
            version: "2.0.0-rc.3".into(),
            files: OWNED_FILES
                .iter()
                .map(|n| ((*n).into(), format!("synthetic {n}\n")))
                .collect(),
        }
    }
    fn count_files(root: &Path) -> usize {
        fs::read_dir(root)
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                if path.is_dir() { count_files(&path) } else { 1 }
            })
            .sum()
    }

    #[test]
    fn bundled_export_has_all_nested_bytes_policy_and_notices() {
        let temp = Temp::new();
        let dest = temp.0.join("中文 export");
        fs::create_dir(&dest).unwrap();
        let package = RimePackage::bundled().unwrap();
        let target = export(&dest, &package).unwrap();
        assert_eq!(count_files(&target), OWNED_FILES.len() + DOCUMENTS.len());
        for (name, text) in &package.files {
            assert_eq!(fs::read(target.join(name)).unwrap(), text.as_bytes());
        }
        for (name, bytes) in DOCUMENTS {
            assert_eq!(fs::read(target.join(name)).unwrap(), *bytes);
        }
        assert!(target.join("lua/xhup_flow/data/quick_hints.lua").is_file());
        assert!(!target.join("xhup_flow_user.userdb").exists());
        assert!(!target.join("user.yaml").exists());
    }

    #[test]
    fn existing_output_is_never_overwritten() {
        let temp = Temp::new();
        let package = package();
        let target = temp.0.join(format!("xhup-flow-rime-v{}", package.version));
        fs::create_dir(&target).unwrap();
        assert!(export(&temp.0, &package).is_err());
        fs::write(target.join("foreign"), b"preserve").unwrap();
        assert!(export(&temp.0, &package).is_err());
        assert_eq!(fs::read(target.join("foreign")).unwrap(), b"preserve");
        fs::remove_dir_all(&target).unwrap();
        fs::write(&target, b"file").unwrap();
        assert!(export(&temp.0, &package).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"file");
    }

    #[test]
    fn bad_inventory_or_version_fails_before_creating_output() {
        let temp = Temp::new();
        for version in ["", "../escape", "1/../../escape", "1:bad", "1\\bad"] {
            let mut package = package();
            package.version = version.into();
            assert!(export(&temp.0, &package).is_err());
        }
        for name in ["../escape", "user.yaml", "/absolute"] {
            let mut package = package();
            package.files[0].0 = name.into();
            assert!(export(&temp.0, &package).is_err());
        }
        let mut p = package();
        p.files[0] = p.files[1].clone();
        assert!(export(&temp.0, &p).is_err());
        assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 0);
    }

    #[test]
    fn write_failure_removes_only_new_export_and_allows_retry() {
        let temp = Temp::new();
        let package = package();
        fs::write(temp.0.join("user.yaml"), b"private").unwrap();
        let mut writes = 0;
        assert!(
            export_with(&temp.0, &package, |path, bytes| {
                writes += 1;
                if writes == 3 {
                    return Err(io::Error::other("injected disk failure"));
                }
                fs::write(path, bytes)
            })
            .is_err()
        );
        assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 1);
        assert_eq!(fs::read(temp.0.join("user.yaml")).unwrap(), b"private");
        assert!(export(&temp.0, &package).is_ok());
    }

    #[test]
    fn concurrent_export_has_exactly_one_winner() {
        let temp = Temp::new();
        let package = package();
        let results = std::thread::scope(|scope| {
            let a = scope.spawn(|| export(&temp.0, &package));
            let b = scope.spawn(|| export(&temp.0, &package));
            [a.join().unwrap().is_ok(), b.join().unwrap().is_ok()]
        });
        assert_eq!(results.into_iter().filter(|ok| *ok).count(), 1);
    }

    #[test]
    fn missing_or_file_destination_is_refused() {
        let temp = Temp::new();
        let path = temp.0.join("missing");
        assert!(export(&path, &package()).is_err());
        fs::write(&path, b"foreign").unwrap();
        assert!(export(&path, &package()).is_err());
        assert_eq!(fs::read(path).unwrap(), b"foreign");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_destination_and_target_are_refused() {
        use std::os::unix::fs::symlink;
        let temp = Temp::new();
        let outside = Temp::new();
        let package = package();
        let link = temp.0.join("link");
        symlink(&outside.0, &link).unwrap();
        assert!(export(&link, &package).is_err());
        let target = temp.0.join(format!("xhup-flow-rime-v{}", package.version));
        symlink(&outside.0, &target).unwrap();
        assert!(export(&temp.0, &package).is_err());
        assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 0);
    }
}
