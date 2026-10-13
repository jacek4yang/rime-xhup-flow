//! Synthetic bytes only: no historical acceptance or user data is modified.
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use xhup_cli::acceptance::{
    self, AcceptanceManifest, CHECK_KEYS, CheckState, PlatformEntry, TestedBuild,
    provenance::{self, BUILD_MANIFEST, BuildManifest},
};

#[path = "support/runtime_qualification.rs"]
mod runtime_qualification;

const SOURCE: &str = "0123456789abcdef0123456789abcdef01234567";
const RC: &str = "2.0.0-rc.3";
const VERSION: &str = "2.0.0";
static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    dir: PathBuf,
    manifest: AcceptanceManifest,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "xhup-acceptance-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        for name in provenance::payload_names(VERSION) {
            fs::write(dir.join(name), b"synthetic release bytes\n").unwrap();
        }
        provenance::seal(VERSION, SOURCE, &dir).unwrap();
        let raw = fs::read(dir.join(BUILD_MANIFEST)).unwrap();
        let build: BuildManifest = serde_json::from_slice(&raw).unwrap();
        let platforms = ["windows", "linux", "macos", "android"]
            .map(|platform| PlatformEntry {
                platform: platform.to_string(),
                frontend: "synthetic-client 1".to_string(),
                os: "synthetic-os 1".to_string(),
                architecture: "test-arch".to_string(),
                artifact: format!("xhup-flow-rime-v{VERSION}.zip"),
                checks: CHECK_KEYS
                    .map(|key| (key.to_string(), CheckState::Pass))
                    .into(),
                evidence: Some("synthetic test evidence; NOT real acceptance".to_string()),
                verified_at: Some("2026-09-27T04:00:00Z".to_string()),
                runtime: Some("synthetic librime/Lua/client versions".to_string()),
                tested_build: Some(TestedBuild {
                    version: VERSION.into(),
                    source_commit: SOURCE.into(),
                    build_manifest_sha256: provenance::digest(&raw),
                }),
                exemptions: BTreeMap::new(),
            })
            .to_vec();
        let manifest = AcceptanceManifest {
            schema_version: 3,
            version: VERSION.to_string(),
            accepted_rc: Some(RC.to_string()),
            source_commit: SOURCE.to_string(),
            build_manifest_sha256: Some(provenance::digest(&raw)),
            artifacts: build.artifacts,
            platforms,
        };
        let fixture = Self { dir, manifest };
        assert!(fixture.verify().is_empty(), "positive control must pass");
        fixture
    }
    fn verify(&self) -> Vec<acceptance::Violation> {
        provenance::verify(&self.manifest, "2.0.0", SOURCE, &self.dir)
    }
    fn cli_code(&self) -> Option<i32> {
        let receipt = self.dir.with_extension("json");
        fs::write(&receipt, serde_json::to_vec(&self.manifest).unwrap()).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_xhup-cli"))
            .args(["validate-acceptance", "--manifest"])
            .arg(&receipt)
            .args([
                "--stable",
                "--expect-version",
                VERSION,
                "--expect-source",
                SOURCE,
                "--artifacts-dir",
            ])
            .arg(&self.dir)
            .output()
            .unwrap();
        fs::remove_file(receipt).unwrap();
        result.status.code()
    }
    fn rebind_manifest(&mut self, raw: &[u8]) {
        let digest = provenance::digest(raw);
        self.manifest.build_manifest_sha256 = Some(digest.clone());
        for platform in &mut self.manifest.platforms {
            platform
                .tested_build
                .as_mut()
                .unwrap()
                .build_manifest_sha256 = digest.clone();
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.dir).unwrap();
    }
}

#[test]
fn fresh_receipt_binds_bytes_without_inheriting_acceptance() {
    let f = Fixture::new();
    let run = |version: &str, source: &str| {
        Command::new(env!("CARGO_BIN_EXE_xhup-cli"))
            .args([
                "init-acceptance",
                "--version",
                version,
                "--source-commit",
                source,
                "--artifacts-dir",
            ])
            .arg(&f.dir)
            .output()
            .unwrap()
    };
    let output = run(VERSION, SOURCE);
    assert_eq!(output.status.code(), Some(0));
    let receipt: AcceptanceManifest = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        receipt.build_manifest_sha256,
        f.manifest.build_manifest_sha256
    );
    assert_eq!(receipt.version, VERSION);
    assert_eq!(receipt.source_commit, SOURCE);
    assert_eq!(receipt.accepted_rc, None);
    for platform in &receipt.platforms {
        assert_eq!(platform.evidence, None);
        assert_eq!(platform.tested_build, None);
        for key in CHECK_KEYS {
            assert_eq!(platform.checks[key], CheckState::Unverified);
        }
    }
    assert_eq!(run(RC, SOURCE).status.code(), Some(1));
    assert_eq!(run(VERSION, &"b".repeat(40)).status.code(), Some(1));
    fs::write(
        f.dir.join(&f.manifest.artifacts[0].name),
        b"changed after seal",
    )
    .unwrap();
    let output = run(VERSION, SOURCE);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"");
}

macro_rules! rejects {
    ($name:ident, $mutate:expr) => {
        #[test]
        fn $name() {
            let mut f = Fixture::new();
            ($mutate)(&mut f.manifest);
            assert_eq!(f.cli_code(), Some(1), "invalid release was not blocked");
        }
    };
}

rejects!(all_na, |m: &mut AcceptanceManifest| {
    for p in &mut m.platforms {
        for s in p.checks.values_mut() {
            *s = CheckState::NotApplicable;
        }
    }
});
rejects!(fake_source, |m: &mut AcceptanceManifest| m.source_commit =
    "not-a-commit".into());
rejects!(
    well_formed_but_unbound_source,
    |m: &mut AcceptanceManifest| m.source_commit = "a".repeat(40)
);
rejects!(unrelated_rc, |m: &mut AcceptanceManifest| m.accepted_rc =
    Some("1.9.0-rc.3".into()));
rejects!(malformed_rc, |m: &mut AcceptanceManifest| m.accepted_rc =
    Some("unrelated-rc.foo".into()));
rejects!(missing_accepted_rc, |m: &mut AcceptanceManifest| m
    .accepted_rc =
    None);
rejects!(missing_evidence, |m: &mut AcceptanceManifest| m.platforms
    [0]
.evidence =
    None);
rejects!(blank_evidence, |m: &mut AcceptanceManifest| m.platforms
    [0]
.evidence =
    Some(" \n ".into()));
rejects!(missing_timestamp, |m: &mut AcceptanceManifest| m
    .platforms[0]
    .verified_at =
    None);
rejects!(invalid_calendar_timestamp, |m: &mut AcceptanceManifest| m
    .platforms[0]
    .verified_at =
    Some("2026-02-30T00:00:00Z".into()));
rejects!(changed_accepted_digest, |m: &mut AcceptanceManifest| m
    .artifacts[0]
    .sha256 =
    "a".repeat(64));
rejects!(unsupported_exemption, |m: &mut AcceptanceManifest| {
    m.platforms[0]
        .checks
        .insert("privacy".into(), CheckState::NotApplicable);
    m.platforms[0]
        .exemptions
        .insert("privacy".into(), "not convenient".into());
});
rejects!(missing_exemption_reason, |m: &mut AcceptanceManifest| {
    m.platforms[3]
        .checks
        .insert("trainer_lifecycle".into(), CheckState::NotApplicable);
});
rejects!(version_mismatch, |m: &mut AcceptanceManifest| m.version =
    "2.0.1".into());
rejects!(
    legacy_schema_not_promotable,
    |m: &mut AcceptanceManifest| m.schema_version = 1
);
rejects!(
    rc_promotion_schema_not_promotable,
    |m: &mut AcceptanceManifest| m.schema_version = 2
);
rejects!(unknown_schema, |m: &mut AcceptanceManifest| m
    .schema_version =
    99);
rejects!(missing_build_identity, |m: &mut AcceptanceManifest| m
    .build_manifest_sha256 =
    None);
rejects!(missing_runtime_identity, |m: &mut AcceptanceManifest| m
    .platforms[0]
    .runtime =
    None);
rejects!(missing_tested_build, |m: &mut AcceptanceManifest| m
    .platforms[0]
    .tested_build =
    None);
rejects!(old_rc_platform_receipt, |m: &mut AcceptanceManifest| m
    .platforms[0]
    .tested_build
    .as_mut()
    .unwrap()
    .version =
    RC.into());
rejects!(wrong_platform_source, |m: &mut AcceptanceManifest| m
    .platforms[0]
    .tested_build
    .as_mut()
    .unwrap()
    .source_commit =
    "b".repeat(40));
rejects!(wrong_platform_build, |m: &mut AcceptanceManifest| m
    .platforms[0]
    .tested_build
    .as_mut()
    .unwrap()
    .build_manifest_sha256 =
    "b".repeat(64));
rejects!(unverified_platform, |m: &mut AcceptanceManifest| {
    m.platforms[0]
        .checks
        .insert("clean_install".into(), CheckState::Unverified);
});
rejects!(failed_platform, |m: &mut AcceptanceManifest| {
    m.platforms[0]
        .checks
        .insert("clean_install".into(), CheckState::Fail);
});
rejects!(duplicate_platform, |m: &mut AcceptanceManifest| m
    .platforms
    .push(m.platforms[0].clone()));
rejects!(unknown_platform, |m: &mut AcceptanceManifest| m.platforms
    [0]
.platform =
    "other".into());
rejects!(duplicate_artifact, |m: &mut AcceptanceManifest| m
    .artifacts
    .push(m.artifacts[0].clone()));
rejects!(missing_required_check, |m: &mut AcceptanceManifest| {
    m.platforms[0].checks.remove("privacy");
});

#[test]
fn current_android_training_lifecycle_cannot_be_exempted() {
    let mut f = Fixture::new();
    f.manifest.platforms[3]
        .checks
        .insert("trainer_lifecycle".into(), CheckState::NotApplicable);
    f.manifest.platforms[3]
        .exemptions
        .insert("trainer_lifecycle".into(), "desktop-only lifecycle".into());
    assert_eq!(f.cli_code(), Some(1));
}

#[test]
fn actual_payload_mutation_is_rejected() {
    let f = Fixture::new();
    fs::write(f.dir.join(&f.manifest.artifacts[0].name), b"changed").unwrap();
    assert_eq!(f.cli_code(), Some(1));
}

#[test]
fn build_manifest_identity_and_source_are_checked() {
    let mut f = Fixture::new();
    let path = f.dir.join(BUILD_MANIFEST);
    let mut build: BuildManifest = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    build.source_commit = "f".repeat(40);
    let raw = serde_json::to_vec(&build).unwrap();
    fs::write(path, &raw).unwrap();
    assert_eq!(f.cli_code(), Some(1));
    f.rebind_manifest(&raw);
    assert_eq!(f.cli_code(), Some(1));
}

#[test]
fn internally_rehashed_wrong_build_version_is_rejected() {
    let mut f = Fixture::new();
    let path = f.dir.join(BUILD_MANIFEST);
    let mut build: BuildManifest = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    build.version = "2.0.0-rc.4".to_string();
    let raw = serde_json::to_vec(&build).unwrap();
    f.rebind_manifest(&raw);
    fs::write(path, raw).unwrap();
    assert_eq!(f.cli_code(), Some(1));
}

#[test]
fn rehashed_partial_artifact_set_is_rejected() {
    let mut f = Fixture::new();
    let path = f.dir.join(BUILD_MANIFEST);
    let mut build: BuildManifest = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let removed = build.artifacts.pop().unwrap();
    fs::remove_file(f.dir.join(removed.name)).unwrap();
    let raw = serde_json::to_vec(&build).unwrap();
    f.rebind_manifest(&raw);
    f.manifest.artifacts = build.artifacts;
    fs::write(path, raw).unwrap();
    assert_eq!(f.cli_code(), Some(1));
}

#[test]
fn missing_and_extra_files_are_rejected() {
    let f = Fixture::new();
    fs::remove_file(f.dir.join(&f.manifest.artifacts[0].name)).unwrap();
    assert_eq!(f.cli_code(), Some(1));
    let f = Fixture::new();
    fs::write(f.dir.join("unexpected.apk"), b"unaccepted").unwrap();
    assert_eq!(f.cli_code(), Some(1));
}

#[cfg(unix)]
#[test]
fn symlink_artifact_is_rejected() {
    let f = Fixture::new();
    let path = f.dir.join(&f.manifest.artifacts[0].name);
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(f.dir.join(&f.manifest.artifacts[1].name), path).unwrap();
    assert_eq!(f.cli_code(), Some(1));
}

#[test]
fn canonical_versions_and_dates() {
    for rc in [
        "2.0.0-rc.0",
        "2.0.0-rc.01",
        "02.0.0-rc.1",
        "2.0.0-rc.1+meta",
        "2.0.0-rc.1.foo",
        "2.0.0",
        " 2.0.0-rc.1",
    ] {
        assert!(provenance::rc_core(rc).is_none(), "{rc}");
    }
    assert_eq!(provenance::rc_core(RC), Some("2.0.0"));
    assert!(provenance::utc_timestamp("2024-02-29T23:59:59Z"));
    for stamp in [
        "2023-02-29T00:00:00Z",
        "2024-00-01T00:00:00Z",
        "2024-01-01T24:00:00Z",
        "",
        "éééééééééé",
    ] {
        assert!(!provenance::utc_timestamp(stamp));
    }
}

#[test]
fn seal_is_deterministic_and_refuses_overwrite() {
    let a = Fixture::new();
    let b = Fixture::new();
    assert_eq!(
        fs::read(a.dir.join(BUILD_MANIFEST)).unwrap(),
        fs::read(b.dir.join(BUILD_MANIFEST)).unwrap()
    );
    assert!(provenance::seal(VERSION, SOURCE, &a.dir).is_err());
}

#[test]
fn cli_requires_external_binding_and_checks_actual_bytes() {
    let f = Fixture::new();
    let acceptance = f.dir.with_extension("json");
    fs::write(&acceptance, serde_json::to_vec(&f.manifest).unwrap()).unwrap();
    let cli = env!("CARGO_BIN_EXE_xhup-cli");
    let base = [
        "validate-acceptance",
        "--manifest",
        acceptance.to_str().unwrap(),
        "--stable",
        "--expect-version",
        "2.0.0",
    ];
    assert_eq!(
        Command::new(cli).args(base).output().unwrap().status.code(),
        Some(2)
    );
    let run = || {
        Command::new(cli)
            .args(base)
            .args([
                "--expect-source",
                SOURCE,
                "--artifacts-dir",
                f.dir.to_str().unwrap(),
            ])
            .output()
            .unwrap()
    };
    assert!(run().status.success());
    fs::write(f.dir.join(&f.manifest.artifacts[0].name), b"tampered").unwrap();
    assert!(!run().status.success());
    fs::remove_file(acceptance).unwrap();
}

#[cfg(unix)]
#[test]
fn workflow_uses_verified_payload() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result = Command::new("python3")
        .arg(root.join("tests/release/test_acceptance_workflow.py"))
        .env("XHUP_CLI", env!("CARGO_BIN_EXE_xhup-cli"))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .expect("python3 required for release workflow contract tests");
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn historical_record_preserved_but_not_stable_evidence() {
    let manifest =
        acceptance::parse_manifest(include_str!("../../../release/acceptance-v2.0.0.json"))
            .unwrap();
    assert!(acceptance::check_rc(&manifest).is_empty());
    assert!(!acceptance::check_stable(&manifest, "2.0.0").is_empty());
}
