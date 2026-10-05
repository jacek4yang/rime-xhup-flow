//! Synthetic bytes only: no historical acceptance or user data is modified.
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use xhup_cli::acceptance::{
    self, AcceptanceManifest, CHECK_KEYS, CheckState, PlatformEntry,
    provenance::{self, BUILD_MANIFEST, BuildManifest},
};

#[path = "support/runtime_qualification.rs"]
mod runtime_qualification;

const SOURCE: &str = "0123456789abcdef0123456789abcdef01234567";
const RC: &str = "2.0.0-rc.3";
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
        for name in provenance::payload_names(RC) {
            fs::write(dir.join(name), b"synthetic release bytes\n").unwrap();
        }
        provenance::seal(RC, SOURCE, &dir).unwrap();
        let raw = fs::read(dir.join(BUILD_MANIFEST)).unwrap();
        let build: BuildManifest = serde_json::from_slice(&raw).unwrap();
        let platforms = ["windows", "linux", "macos", "android"]
            .map(|platform| PlatformEntry {
                platform: platform.to_string(),
                frontend: "synthetic-client 1".to_string(),
                os: "synthetic-os 1".to_string(),
                architecture: "test-arch".to_string(),
                artifact: format!("xhup-flow-rime-v{RC}.zip"),
                checks: CHECK_KEYS
                    .map(|key| (key.to_string(), CheckState::Pass))
                    .into(),
                evidence: Some("synthetic test evidence; NOT real acceptance".to_string()),
                verified_at: Some("2026-09-27T04:00:00Z".to_string()),
                runtime: Some("synthetic librime/Lua/client versions".to_string()),
                exemptions: BTreeMap::new(),
            })
            .to_vec();
        let manifest = AcceptanceManifest {
            schema_version: 2,
            version: "2.0.0".to_string(),
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
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.dir).unwrap();
    }
}

macro_rules! rejects {
    ($name:ident, $mutate:expr, $message:expr) => {
        #[test]
        fn $name() {
            let mut f = Fixture::new();
            ($mutate)(&mut f.manifest);
            let violations = f.verify();
            assert!(
                violations.iter().any(|v| v.message.contains($message)),
                "expected {}, got {violations:?}",
                $message
            );
        }
    };
}

rejects!(
    all_na,
    |m: &mut AcceptanceManifest| {
        for p in &mut m.platforms {
            for s in p.checks.values_mut() {
                *s = CheckState::NotApplicable;
            }
        }
    },
    "N/A"
);
rejects!(
    fake_source,
    |m: &mut AcceptanceManifest| m.source_commit = "not-a-commit".into(),
    "source_commit"
);
rejects!(
    well_formed_but_unbound_source,
    |m: &mut AcceptanceManifest| m.source_commit = "a".repeat(40),
    "source_commit"
);
rejects!(
    unrelated_rc,
    |m: &mut AcceptanceManifest| m.accepted_rc = Some("1.9.0-rc.3".into()),
    "不是 RC"
);
rejects!(
    malformed_rc,
    |m: &mut AcceptanceManifest| m.accepted_rc = Some("unrelated-rc.foo".into()),
    "不是 RC"
);
rejects!(
    missing_evidence,
    |m: &mut AcceptanceManifest| m.platforms[0].evidence = None,
    "evidence"
);
rejects!(
    blank_evidence,
    |m: &mut AcceptanceManifest| m.platforms[0].evidence = Some(" \n ".into()),
    "evidence"
);
rejects!(
    missing_timestamp,
    |m: &mut AcceptanceManifest| m.platforms[0].verified_at = None,
    "verified_at"
);
rejects!(
    invalid_calendar_timestamp,
    |m: &mut AcceptanceManifest| m.platforms[0].verified_at = Some("2026-02-30T00:00:00Z".into()),
    "verified_at"
);
rejects!(
    changed_accepted_digest,
    |m: &mut AcceptanceManifest| m.artifacts[0].sha256 = "a".repeat(64),
    "artifact set/digests"
);
rejects!(
    unsupported_exemption,
    |m: &mut AcceptanceManifest| {
        m.platforms[0]
            .checks
            .insert("privacy".into(), CheckState::NotApplicable);
        m.platforms[0]
            .exemptions
            .insert("privacy".into(), "not convenient".into());
    },
    "N/A"
);
rejects!(
    missing_exemption_reason,
    |m: &mut AcceptanceManifest| {
        m.platforms[3]
            .checks
            .insert("trainer_lifecycle".into(), CheckState::NotApplicable);
    },
    "N/A"
);
rejects!(
    version_mismatch,
    |m: &mut AcceptanceManifest| m.version = "2.0.1".into(),
    "不一致"
);
rejects!(
    legacy_schema_not_promotable,
    |m: &mut AcceptanceManifest| m.schema_version = 1,
    "schema_version 2"
);
rejects!(
    unknown_schema,
    |m: &mut AcceptanceManifest| m.schema_version = 99,
    "schema_version"
);
rejects!(
    missing_build_identity,
    |m: &mut AcceptanceManifest| m.build_manifest_sha256 = None,
    "build_manifest_sha256"
);
rejects!(
    missing_runtime_identity,
    |m: &mut AcceptanceManifest| m.platforms[0].runtime = None,
    "runtime"
);
rejects!(
    unverified_platform,
    |m: &mut AcceptanceManifest| {
        m.platforms[0]
            .checks
            .insert("clean_install".into(), CheckState::Unverified);
    },
    "UNVERIFIED"
);
rejects!(
    failed_platform,
    |m: &mut AcceptanceManifest| {
        m.platforms[0]
            .checks
            .insert("clean_install".into(), CheckState::Fail);
    },
    "FAIL"
);
rejects!(
    duplicate_platform,
    |m: &mut AcceptanceManifest| m.platforms.push(m.platforms[0].clone()),
    "必须恰好一次"
);
rejects!(
    unknown_platform,
    |m: &mut AcceptanceManifest| m.platforms[0].platform = "other".into(),
    "未知平台"
);
rejects!(
    duplicate_artifact,
    |m: &mut AcceptanceManifest| m.artifacts.push(m.artifacts[0].clone()),
    "工件名重复"
);
rejects!(
    missing_required_check,
    |m: &mut AcceptanceManifest| {
        m.platforms[0].checks.remove("privacy");
    },
    "缺少必查项"
);

#[test]
fn explicitly_reasoned_android_exemption_passes() {
    let mut f = Fixture::new();
    f.manifest.platforms[3]
        .checks
        .insert("trainer_lifecycle".into(), CheckState::NotApplicable);
    f.manifest.platforms[3]
        .exemptions
        .insert("trainer_lifecycle".into(), "desktop-only lifecycle".into());
    assert!(f.verify().is_empty());
}

#[test]
fn actual_payload_mutation_is_rejected() {
    let f = Fixture::new();
    fs::write(f.dir.join(&f.manifest.artifacts[0].name), b"changed").unwrap();
    assert!(
        f.verify()
            .iter()
            .any(|v| v.message.contains("artifact digest mismatch"))
    );
}

#[test]
fn build_manifest_identity_and_source_are_checked() {
    let f = Fixture::new();
    let path = f.dir.join(BUILD_MANIFEST);
    let mut build: BuildManifest = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    build.source_commit = "f".repeat(40);
    fs::write(path, serde_json::to_vec(&build).unwrap()).unwrap();
    let violations = f.verify();
    assert!(
        violations
            .iter()
            .any(|v| v.message.contains("build_manifest_sha256 mismatch"))
    );
    assert!(
        violations
            .iter()
            .any(|v| v.message.contains("schema/version/source"))
    );
}

#[test]
fn internally_rehashed_wrong_build_version_is_rejected() {
    let mut f = Fixture::new();
    let path = f.dir.join(BUILD_MANIFEST);
    let mut build: BuildManifest = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    build.version = "2.0.0-rc.4".to_string();
    let raw = serde_json::to_vec(&build).unwrap();
    f.manifest.build_manifest_sha256 = Some(provenance::digest(&raw));
    fs::write(path, raw).unwrap();
    assert!(
        f.verify()
            .iter()
            .any(|v| v.message.contains("schema/version/source"))
    );
}

#[test]
fn rehashed_partial_artifact_set_is_rejected() {
    let mut f = Fixture::new();
    let path = f.dir.join(BUILD_MANIFEST);
    let mut build: BuildManifest = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let removed = build.artifacts.pop().unwrap();
    fs::remove_file(f.dir.join(removed.name)).unwrap();
    let raw = serde_json::to_vec(&build).unwrap();
    f.manifest.build_manifest_sha256 = Some(provenance::digest(&raw));
    f.manifest.artifacts = build.artifacts;
    fs::write(path, raw).unwrap();
    assert!(
        f.verify()
            .iter()
            .any(|v| v.message.contains("exact complete"))
    );
}

#[test]
fn missing_and_extra_files_are_rejected() {
    let f = Fixture::new();
    fs::remove_file(f.dir.join(&f.manifest.artifacts[0].name)).unwrap();
    assert!(!f.verify().is_empty());
    fs::write(f.dir.join("unexpected.apk"), b"unaccepted").unwrap();
    assert!(
        f.verify()
            .iter()
            .any(|v| v.message.contains("unexpected assets"))
    );
}

#[cfg(unix)]
#[test]
fn symlink_artifact_is_rejected() {
    let f = Fixture::new();
    let path = f.dir.join(&f.manifest.artifacts[0].name);
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(f.dir.join(&f.manifest.artifacts[1].name), path).unwrap();
    assert!(f.verify().iter().any(|v| v.message.contains("not symlink")));
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
    assert!(provenance::seal(RC, SOURCE, &a.dir).is_err());
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
