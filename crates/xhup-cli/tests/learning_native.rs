//! Real rime_dict_manager integration. Explicit opt-in; CI installs and runs it.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use xhup_cli::learning;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "xhup-native-import-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn snapshot(identity: &str, version: &str) -> String {
    format!(
        "# Rime user dictionary\n#@/db_name\t{identity}\n#@/db_type\tuserdb\n#@/rime_version\t{version}\n#@/tick\t1\njb zq \t进去\tc=1 d=1 t=1\n"
    )
}
fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, path: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(root, root, &mut out);
    out
}

#[test]
#[ignore = "requires real rime_dict_manager; run explicitly in librime CI"]
fn import_ownership_native_roundtrip_and_historical_migration() {
    let manager = learning::which_dict_manager().expect("mandatory native tool missing");
    let fixture = Fixture::new();
    let user = fixture.0.join("user");
    let input = fixture.0.join("input");
    fs::create_dir(&user).unwrap();
    fs::create_dir(&input).unwrap();
    let source = input.join("xhup_flow_user.userdb.txt");

    // Audit reproduction must not even create a foreign database.
    fs::write(&source, snapshot("foreign_audit_dictionary", "1.16.1")).unwrap();
    assert!(learning::import(&user, &source, Some(&manager)).is_err());
    assert!(tree(&user).is_empty());

    // Seed a genuine unrelated DB using librime directly, then ensure byte identity.
    let output = Command::new(&manager)
        .current_dir(&user)
        .arg("-r")
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "native fixture creation failed");
    let foreign = user.join("foreign_audit_dictionary.userdb");
    assert!(foreign.is_dir());
    let before = tree(&foreign);
    assert!(!before.is_empty());
    for text in [
        snapshot("foreign_audit_dictionary", "1.16.1"),
        snapshot("xhup_flow_user", "99.0.0"),
        snapshot("xhup_flow_user", "1.16.1").replace("#@/db_name\txhup_flow_user\n", ""),
        snapshot("xhup_flow_user", "1.16.1").replace("#@/db_type\tuserdb", "#@/db_type bad"),
    ] {
        fs::write(&source, text).unwrap();
        assert!(learning::import(&user, &source, Some(&manager)).is_err());
        assert_eq!(tree(&foreign), before);
        assert!(!user.join("xhup_flow_user.userdb").exists());
    }
    // An actual accepted historical format is restored and re-exported by native Rime.
    fs::write(&source, snapshot("xhup_flow_user.userdb.kct", "1.8.5")).unwrap();
    learning::import(&user, &source, Some(&manager)).unwrap();
    assert_eq!(tree(&foreign), before);
    let backup = learning::export(&user, None, Some(&manager)).unwrap();
    let exported = fs::read_to_string(&backup).unwrap();
    assert!(
        exported.contains("进去"),
        "native restored entry must be exportable"
    );
    learning::import(&user, &backup, Some(&manager)).unwrap();
    assert_eq!(tree(&foreign), before);

    // librime removes a fixed .temp DB during restore; refuse any preexisting one.
    let temp = user.join(".temp.userdb");
    fs::create_dir(&temp).unwrap();
    fs::write(temp.join("sentinel"), b"unowned").unwrap();
    assert!(learning::import(&user, &source, Some(&manager)).is_err());
    assert_eq!(fs::read(temp.join("sentinel")).unwrap(), b"unowned");
    assert_eq!(tree(&foreign), before);
}

#[test]
#[ignore = "requires real rime_dict_manager; run explicitly in librime CI"]
fn renamed_foreign_database_is_never_modified_or_deleted() {
    let manager = learning::which_dict_manager().unwrap();
    let fixture = Fixture::new();
    let user = fixture.0.join("user");
    fs::create_dir(&user).unwrap();
    let source = fixture.0.join("xhup_flow_user.userdb.txt");
    fs::write(&source, snapshot("foreign_audit_dictionary", "1.16.1")).unwrap();
    assert!(
        Command::new(&manager)
            .current_dir(&user)
            .arg("-r")
            .arg(&source)
            .output()
            .unwrap()
            .status
            .success()
    );
    let disguised = user.join("xhup_flow_user.userdb");
    fs::rename(user.join("foreign_audit_dictionary.userdb"), &disguised).unwrap();
    let before = tree(&disguised);
    fs::write(&source, snapshot("xhup_flow_user", "1.16.1")).unwrap();
    assert!(learning::export(&user, None, Some(&manager)).is_err());
    assert!(learning::import(&user, &source, Some(&manager)).is_err());
    assert!(learning::reset(&user, true).is_err());
    assert_eq!(
        tree(&disguised),
        before,
        "ownership inspection must not open original in librime"
    );
    assert!(!user.join("xhup_flow_user.userdb.txt").exists());
    assert!(!user.join("foreign_audit_dictionary.userdb").exists());
    #[cfg(unix)]
    {
        let linked_user = fixture.0.join("linked");
        fs::create_dir(&linked_user).unwrap();
        std::os::unix::fs::symlink(&disguised, linked_user.join("xhup_flow_user.userdb")).unwrap();
        assert!(learning::export(&linked_user, None, Some(&manager)).is_err());
        assert!(learning::import(&linked_user, &source, Some(&manager)).is_err());
        assert!(learning::reset(&linked_user, true).is_err());
        assert_eq!(tree(&disguised), before);
    }
}

#[cfg(unix)]
#[test]
#[ignore = "requires real rime_dict_manager and Python POSIX lock holder"]
fn active_native_lock_blocks_all_management_then_safe_reset_preserves_context() {
    use std::time::{Duration, Instant};
    struct ChildGuard(std::process::Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let manager = learning::which_dict_manager().unwrap();
    let fixture = Fixture::new();
    let user = fixture.0.join("user");
    fs::create_dir(&user).unwrap();
    let source = fixture.0.join("xhup_flow_user.userdb.txt");
    fs::write(&source, snapshot("xhup_flow_user", "1.16.1")).unwrap();
    learning::import(&user, &source, Some(&manager)).unwrap();
    let db = user.join("xhup_flow_user.userdb");
    let before = tree(&db);
    let ready = fixture.0.join("ready");
    let child = ChildGuard(Command::new("python3").arg("-c")
        .arg("import fcntl,sys,time; f=open(sys.argv[1],'r+b'); fcntl.lockf(f,fcntl.LOCK_EX|fcntl.LOCK_NB); open(sys.argv[2],'w').close(); time.sleep(30)")
        .arg(db.join("LOCK")).arg(&ready).spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready.exists() {
        assert!(
            Instant::now() < deadline,
            "external lock holder failed to start"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(learning::export(&user, None, Some(&manager)).is_err());
    assert!(learning::import(&user, &source, Some(&manager)).is_err());
    assert!(learning::reset(&user, true).is_err());
    assert_eq!(tree(&db), before);
    drop(child);

    // Export never opens the original in a manager or rewrites its metadata.
    let exported = learning::export(&user, None, Some(&manager)).unwrap();
    assert!(fs::read_to_string(&exported).unwrap().contains("进去"));
    assert_eq!(tree(&db), before);
    // Never overwrite an unrelated/invalid selected export file.
    fs::write(&exported, b"unowned").unwrap();
    assert!(learning::export(&user, None, Some(&manager)).is_err());
    assert_eq!(fs::read(&exported).unwrap(), b"unowned");

    let contextual = user.join("xhup_flow_context.tsv");
    fs::write(&contextual, b"separate context state").unwrap();
    learning::reset(&user, true).unwrap();
    assert!(!db.exists());
    assert_eq!(fs::read(&contextual).unwrap(), b"separate context state");
    assert_eq!(fs::read(&exported).unwrap(), b"unowned");
    learning::reset(&user, true).unwrap();
    assert!(!fs::read_dir(&user).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".xhup-learning-")
    }));
}
