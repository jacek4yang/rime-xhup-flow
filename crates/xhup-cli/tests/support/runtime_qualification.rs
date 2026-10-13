// Synthetic proof bytes ONLY, not actual platform acceptance.
use super::*;
use serde_json::{Value, json};
use xhup_cli::acceptance::runtime_qualification::{self as policy, Authorization, Context};
struct Qualified {
    fixture: Fixture,
    proofs: PathBuf,
    authorization: Authorization,
}
impl Qualified {
    fn new() -> Self {
        let fixture = Fixture::new();
        let proofs = fixture.dir.with_extension("proofs");
        fs::create_dir(&proofs).unwrap();
        let authorization = Authorization {
            schema_version: 2,
            policy: policy::POLICY.into(),
            version: "2.0.0".into(),
            accepted_rc: RC.into(),
            source_commit: SOURCE.into(),
            build_manifest_sha256: fixture.manifest.build_manifest_sha256.clone().unwrap(),
            repository: "owner/repo".into(),
            approved_by: "owner".into(),
            approved_at: "2026-10-05T00:00:00Z".into(),
            rationale: "SYNTHETIC TEST ONLY".into(),
            limitations: vec!["Synthetic unsigned desktop packages".into()],
            ci_run: 11,
            runtime_run: 12,
            package_run: 13,
        };
        for (name, id, workflow) in [
            ("ci", 11, "ci"),
            ("runtime", 12, "full-regression"),
            ("package", 13, "product-packaging"),
        ] {
            fs::write(proofs.join(format!("{name}-run.json")),json!({"id":id,"run_attempt":1,"status":"completed","conclusion":"success","head_sha":SOURCE,"head_branch":"main","path":format!(".github/workflows/{workflow}.yml"),"event":"workflow_dispatch","repository":{"full_name":"owner/repo","owner":{"login":"owner"}}}).to_string()).unwrap();
        }
        let jobs:Vec<_> = ["Rust workspace","trainer 前端","librime runtime smoke","原生冒烟(ubuntu-latest)","原生冒烟(macos-latest)","原生冒烟(windows-latest)"]
            .map(|name|json!({"name":name,"status":"completed","conclusion":"success","head_sha":SOURCE,"run_id":11})).into();
        fs::write(
            proofs.join("ci-jobs.json"),
            json!({"total_count":6,"jobs":jobs}).to_string(),
        )
        .unwrap();
        fs::write(proofs.join("coverage.json"),json!({"version":1,"revision":SOURCE,"run":"12.1","plan_sha256":"a".repeat(64),"partitions_completed":16,"rows":{"static":141138,"extended":1301434,"open":1000}}).to_string()).unwrap();
        Self {
            fixture,
            proofs,
            authorization,
        }
    }
    fn verify(&self) -> Vec<acceptance::Violation> {
        policy::verify(
            &self.fixture.manifest,
            &self.authorization,
            &Context {
                version: "2.0.0",
                source: SOURCE,
                artifacts: &self.fixture.dir,
                proofs: &self.proofs,
                repository: "owner/repo",
                actor: "owner",
            },
        )
    }
    fn mutate(&self, name: &str, mutate: impl FnOnce(&mut Value)) {
        let path = self.proofs.join(name);
        let mut v = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        mutate(&mut v);
        fs::write(path, v.to_string()).unwrap();
    }
}
impl Drop for Qualified {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.proofs).unwrap();
    }
}
#[test]
fn qualification_never_excuses_unverified_platforms() {
    let mut f = Qualified::new();
    assert!(f.verify().is_empty(), "{:?}", f.verify());
    f.fixture.manifest.platforms[0]
        .checks
        .insert(CHECK_KEYS[0].into(), CheckState::Unverified);
    assert!(
        f.verify()
            .iter()
            .any(|violation| violation.platform.as_deref() == Some("windows")
                && violation.check.as_deref() == Some(CHECK_KEYS[0]))
    );
}
#[test]
fn rejects_proof_mutations() {
    for (file, pointer, value) in [
        ("ci-run.json", "/head_sha", json!("b".repeat(40))),
        ("ci-run.json", "/repository/owner/login", json!("someone")),
        ("ci-run.json", "/repository/full_name", json!("owner/fork")),
        ("ci-run.json", "/head_branch", json!("feature")),
        ("ci-run.json", "/event", json!("pull_request")),
        ("ci-run.json", "/path", json!(".github/workflows/other.yml")),
        ("runtime-run.json", "/conclusion", json!("failure")),
        ("package-run.json", "/status", json!("in_progress")),
        ("package-run.json", "/id", json!(99)),
        ("ci-jobs.json", "/total_count", json!(7)),
        ("ci-jobs.json", "/jobs/5/conclusion", json!("skipped")),
        ("ci-jobs.json", "/jobs/3/head_sha", json!("b".repeat(40))),
        ("coverage.json", "/partitions_completed", json!(15)),
        ("coverage.json", "/run", json!("12.2")),
        ("coverage.json", "/revision", json!("b".repeat(40))),
        ("coverage.json", "/rows/open", json!(999)),
    ] {
        let f = Qualified::new();
        f.mutate(file, |v| *v.pointer_mut(pointer).unwrap() = value);
        assert!(!f.verify().is_empty(), "accepted {file}:{pointer}");
    }
}
#[test]
fn rejects_authorization_mutations() {
    for mutate in [
        (|a: &mut Authorization| a.approved_by = "someone".into()) as fn(&mut Authorization),
        |a| a.policy = "ignore".into(),
        |a| a.limitations.clear(),
        |a| a.source_commit = "b".repeat(40),
        |a| a.approved_at = "2026-02-30T00:00:00Z".into(),
        |a| a.schema_version = 1,
        |a| a.policy = "runtime-qualified-user-platform-testing-v1".into(),
        |a| a.accepted_rc = "2.0.0-rc.2".into(),
        |a| a.build_manifest_sha256 = "b".repeat(64),
    ] {
        let mut f = Qualified::new();
        mutate(&mut f.authorization);
        assert!(!f.verify().is_empty());
    }
}
#[test]
fn cannot_excuse_fail_or_modified_payload() {
    let mut f = Qualified::new();
    f.fixture.manifest.platforms[1]
        .checks
        .insert(CHECK_KEYS[0].into(), CheckState::Fail);
    assert!(!f.verify().is_empty());
    let f = Qualified::new();
    fs::write(
        f.fixture
            .dir
            .join(provenance::payload_names(VERSION)[0].clone()),
        b"changed",
    )
    .unwrap();
    assert!(!f.verify().is_empty());
}
#[test]
fn missing_and_malformed_proofs_fail_closed() {
    for name in [
        "ci-run.json",
        "runtime-run.json",
        "package-run.json",
        "ci-jobs.json",
        "coverage.json",
    ] {
        let f = Qualified::new();
        fs::remove_file(f.proofs.join(name)).unwrap();
        assert!(!f.verify().is_empty());
        fs::write(f.proofs.join(name), b"{}").unwrap();
        assert!(!f.verify().is_empty());
    }
}
