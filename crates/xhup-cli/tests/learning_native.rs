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
