//! Explicit owner-authorized runtime qualification; never relabel platform tests.
//! Full-platform promotion remains the default. This verifier additionally binds
//! successful same-source GitHub runs and independently recollected full coverage.
use super::{AcceptanceManifest, CheckState, Violation, check_stable_policy, provenance};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::Path;

/// A deliberately named policy, not a generic "ignore verification" switch.
pub const POLICY: &str = "runtime-qualified-user-platform-testing-v1";

/// Reviewed release decision. It does not manufacture any PASS evidence.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Authorization {
    pub schema_version: u32,
    pub policy: String,
    pub version: String,
    pub accepted_rc: String,
    pub source_commit: String,
    pub build_manifest_sha256: String,
    pub repository: String,
    pub approved_by: String,
    pub approved_at: String,
    pub rationale: String,
    pub limitations: Vec<String>,
    pub user_testing_platforms: Vec<String>,
    pub ci_run: u64,
    pub runtime_run: u64,
    pub package_run: u64,
}

/// Independently supplied values from the publishing workflow, not the decision JSON.
pub struct Context<'a> {
    pub version: &'a str,
    pub source: &'a str,
    pub artifacts: &'a Path,
    pub proofs: &'a Path,
    pub repository: &'a str,
    pub actor: &'a str,
}

#[derive(Deserialize)]
struct Owner {
    login: String,
}
#[derive(Deserialize)]
struct Repository {
    full_name: String,
    owner: Owner,
}
#[derive(Deserialize)]
struct Run {
    id: u64,
    run_attempt: u64,
    status: String,
    conclusion: Option<String>,
    head_sha: String,
    head_branch: String,
    path: String,
    event: String,
    repository: Repository,
}
#[derive(Deserialize)]
struct Job {
    name: String,
    status: String,
    conclusion: Option<String>,
    head_sha: String,
    run_id: u64,
}
#[derive(Deserialize)]
struct Jobs {
    total_count: usize,
    jobs: Vec<Job>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Coverage {
    version: u32,
    revision: String,
    run: String,
    plan_sha256: String,
    partitions_completed: u64,
    rows: CoverageRows,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CoverageRows {
    #[serde(rename = "static")]
    static_rows: u64,
    extended: u64,
    open: u64,
}

fn require(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.to_owned())
    }
}
fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let limit = 4 * 1024 * 1024;
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    require(
        metadata.is_file()
            && !metadata.file_type().is_symlink()
            && metadata.len() > 0
            && metadata.len() <= limit,
        "proof must be a bounded ordinary JSON file",
    )?;
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    require(bytes.len() as u64 <= limit, "oversized proof")?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}
fn check_run(path: &Path, id: u64, workflow: &str, context: &Context<'_>) -> Result<Run, String> {
    let run: Run = read_json(path)?;
    require(
        run.id == id && id > 0 && run.run_attempt > 0,
        "proof run identity mismatch",
    )?;
    require(
        run.status == "completed" && run.conclusion.as_deref() == Some("success"),
        "required workflow has not completed successfully",
    )?;
    require(
        run.head_sha == context.source && run.head_branch == "main",
        "workflow source/branch is not the accepted main revision",
    )?;
    require(run.path == workflow, "unexpected qualification workflow")?;
    require(
        matches!(run.event.as_str(), "push" | "workflow_dispatch"),
        "qualification cannot use a pull-request/fork/scheduled run",
    )?;
    require(
        run.repository.full_name == context.repository,
        "workflow repository mismatch",
    )?;
    require(
        run.repository.owner.login == context.actor,
        "only the repository owner may authorize pending user platform testing",
    )?;
    Ok(run)
}
fn check(
    manifest: &AcceptanceManifest,
    authorization: &Authorization,
    context: &Context<'_>,
) -> Result<(), String> {
    let a = authorization;
    require(
        a.schema_version == 1 && a.policy == POLICY,
        "unsupported release qualification policy",
    )?;
    require(
        a.version == context.version && manifest.version == context.version,
        "decision version mismatch",
    )?;
    require(
        manifest.accepted_rc.as_deref() == Some(a.accepted_rc.as_str()),
        "decision RC mismatch",
    )?;
    require(
        a.source_commit == context.source && manifest.source_commit == context.source,
        "decision source mismatch",
    )?;
    require(
        manifest.build_manifest_sha256.as_deref() == Some(a.build_manifest_sha256.as_str()),
        "decision build manifest binding mismatch",
    )?;
    require(
        a.repository == context.repository
            && a.approved_by == context.actor
            && !context.actor.is_empty(),
        "decision owner/repository mismatch",
    )?;
    require(
        provenance::utc_timestamp(&a.approved_at),
        "decision needs a valid UTC approval time",
    )?;
    require(
        !a.rationale.trim().is_empty()
            && !a.limitations.is_empty()
            && a.limitations.iter().all(|s| !s.trim().is_empty()),
        "explicit rationale and disclosed limitations are required",
    )?;
    let pending: BTreeSet<_> = manifest
        .platforms
        .iter()
        .filter(|p| p.checks.values().any(|s| *s == CheckState::Unverified))
        .map(|p| p.platform.as_str())
        .collect();
    let accepted: BTreeSet<_> = a
        .user_testing_platforms
        .iter()
        .map(String::as_str)
        .collect();
    require(
        !pending.is_empty()
            && pending == accepted
            && accepted.len() == a.user_testing_platforms.len(),
        "decision must explicitly list exactly the still-unverified platforms; use full-platform mode otherwise",
    )?;
    // A known acceptance failure is never excused by delegating future user tests.
    require(
        !manifest
            .platforms
            .iter()
            .any(|p| p.checks.values().any(|s| *s == CheckState::Fail)),
        "FAIL cannot be overridden by this qualification policy",
    )?;
    let ci = check_run(
        &context.proofs.join("ci-run.json"),
        a.ci_run,
        ".github/workflows/ci.yml",
        context,
    )?;
    let runtime = check_run(
        &context.proofs.join("runtime-run.json"),
        a.runtime_run,
        ".github/workflows/full-regression.yml",
        context,
    )?;
    check_run(
        &context.proofs.join("package-run.json"),
        a.package_run,
        ".github/workflows/xhup-flow-rc-release.yml",
        context,
    )?;
    require(
        a.ci_run != a.runtime_run && a.runtime_run != a.package_run && a.ci_run != a.package_run,
        "qualification runs must be distinct",
    )?;
    let jobs: Jobs = read_json(&context.proofs.join("ci-jobs.json"))?;
    require(
        jobs.jobs.len() == jobs.total_count,
        "truncated CI job listing",
    )?;
    for name in [
        "Rust workspace",
        "trainer 前端",
        "librime runtime smoke",
        "原生冒烟(ubuntu-latest)",
        "原生冒烟(macos-latest)",
        "原生冒烟(windows-latest)",
    ] {
        let found: Vec<_> = jobs.jobs.iter().filter(|j| j.name == name).collect();
        require(
            found.len() == 1
                && found[0].status == "completed"
                && found[0].conclusion.as_deref() == Some("success")
                && found[0].run_id == ci.id
                && found[0].head_sha == context.source,
            "a mandatory CI job is missing, duplicated, failed, pending or from another revision",
        )?;
    }
    // The workflow must produce this file by rerunning audit_shards.py collect
    // over downloaded input + ALL sixteen logs/receipts, not by trusting a table.
    let coverage: Coverage = read_json(&context.proofs.join("coverage.json"))?;
    require(
        coverage.version == 1
            && coverage.revision == context.source
            && coverage.run == format!("{}.{}", runtime.id, runtime.run_attempt),
        "full-audit receipt provenance mismatch",
    )?;
    require(
        coverage.partitions_completed == 16
            && provenance::is_hex(&coverage.plan_sha256, 64)
            && coverage.rows.static_rows > 0
            && coverage.rows.extended > 0
            && coverage.rows.open >= 1000,
        "complete native audit coverage is required",
    )?;
    Ok(())
}

/// Verify explicit scope, external run evidence, RC provenance and actual bytes.
/// Returns all failures; callers must never publish unless this is empty.
pub fn verify(
    manifest: &AcceptanceManifest,
    authorization: &Authorization,
    context: &Context<'_>,
) -> Vec<Violation> {
    let decision = check(manifest, authorization, context);
    let mut violations = check_stable_policy(manifest, context.version, decision.is_ok());
    if let Err(message) = decision {
        violations.push(Violation {
            platform: None,
            check: None,
            message,
        });
    }
    provenance::verify_with_checks(manifest, context.source, context.artifacts, violations)
}
