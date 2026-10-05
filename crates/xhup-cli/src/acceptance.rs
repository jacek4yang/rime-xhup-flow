//! GA 验收清单(`release/acceptance-v2.0.0.json`)的类型与确定性校验。
//!
//! 设计约束(#148 §2):
//! - 状态严格有限:`PASS` / `FAIL` / `UNVERIFIED` / `N/A`,解析拒绝任何其他值;
//! - 策略校验是纯函数;provenance 校验另外读取实际附件,均不联网;
//! - stable 必须使用 provenance::verify:check_stable 仅校验 schema 2 策略;
//!   RC 门禁语义:`check_rc` 允许 UNVERIFIED,但 FAIL 仍然阻止。
//!
//! 文档同步(`docs/platform-acceptance.md`)由工作流调用本模块的
//! render_markdown 生成比对,保证文档不与清单漂移。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

pub mod provenance;
pub mod runtime_qualification;

/// 单项验收状态。严格有限,反序列化拒绝未知值(malformed 用例依赖此点)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckState {
    /// 真机/等价环境验证通过,证据已记录。
    #[serde(rename = "PASS")]
    Pass,
    /// 验证执行且失败 —— 稳定版阻塞项。
    #[serde(rename = "FAIL")]
    Fail,
    /// 尚未验证 —— 稳定版阻塞项(不冒充通过)。
    #[serde(rename = "UNVERIFIED")]
    Unverified,
    /// 该平台不适用(如 Android 无桌面 Trainer 控制中心)。
    #[serde(rename = "N/A")]
    NotApplicable,
}

impl CheckState {
    pub fn as_str(self) -> &'static str {
        match self {
            CheckState::Pass => "PASS",
            CheckState::Fail => "FAIL",
            CheckState::Unverified => "UNVERIFIED",
            CheckState::NotApplicable => "N/A",
        }
    }
}

impl fmt::Display for CheckState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 十二项核心行为检查的键(#148 §4)。
pub const CHECK_KEYS: [&str; 12] = [
    "clean_install",
    "upgrade_from_v1",
    "schema_deployment",
    "static_smoke",
    "flow_sentence_input",
    "learning_persistence",
    "oov_reachability",
    "context_ranker_neutrality",
    "joint_decoder_neutrality",
    "static_fallback",
    "trainer_lifecycle",
    "privacy",
];

/// 发布工件名 + SHA256(小写 hex)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub name: String,
    pub sha256: String,
}

/// 单平台验收条目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformEntry {
    pub platform: String,
    pub frontend: String,
    pub os: String,
    pub architecture: String,
    /// 该平台验收所用的安装工件名(必须出现在顶层 artifacts 中)。
    pub artifact: String,
    pub checks: BTreeMap<String, CheckState>,
    /// 证据链接/描述(PR 评论、日志、截图路径);PASS 时必填。
    pub evidence: Option<String>,
    /// 验证时间戳(ISO 8601);PASS 时必填。
    pub verified_at: Option<String>,
    /// 明确的 librime / Lua / 客户端版本;schema 2 stable 必填。
    #[serde(default)]
    pub runtime: Option<String>,
    /// 仅允许 Android/trainer_lifecycle;schema 2 必须给出非空原因。
    #[serde(default)]
    pub exemptions: BTreeMap<String, String>,
}

/// 机器可读验收清单。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceManifest {
    pub schema_version: u32,
    pub version: String,
    /// GA 发布所依据的已验收 RC 版本;stable 时必填。
    pub accepted_rc: Option<String>,
    pub source_commit: String,
    /// 已验收 BUILD-MANIFEST.json 原始字节的 SHA256;stable 必填。
    #[serde(default)]
    pub build_manifest_sha256: Option<String>,
    pub artifacts: Vec<ArtifactRecord>,
    pub platforms: Vec<PlatformEntry>,
}

/// 校验失败的精确原因(工作流失败输出直接展示)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub platform: Option<String>,
    pub check: Option<String>,
    pub message: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.platform, &self.check) {
            (Some(p), Some(c)) => write!(f, "{p}/{c}: {}", self.message),
            (Some(p), None) => write!(f, "{p}: {}", self.message),
            _ => write!(f, "{}", self.message),
        }
    }
}

/// 结构校验:不关心状态取值,只关心清单本身合法。
/// 违例按确定性顺序(BTreeMap 键序 + 平台数组序)返回。
pub fn validate_structure(manifest: &AcceptanceManifest) -> Vec<Violation> {
    let mut violations = Vec::new();

    if ![1, 2].contains(&manifest.schema_version) {
        violations.push(Violation {
            platform: None,
            check: None,
            message: format!(
                "schema_version 必须为 1 或 2,实际 {}",
                manifest.schema_version
            ),
        });
    }
    if manifest.version.is_empty() {
        violations.push(Violation {
            platform: None,
            check: None,
            message: "version 不能为空".to_string(),
        });
    }
    if !provenance::is_hex(&manifest.source_commit, 40) {
        violations.push(Violation {
            platform: None,
            check: None,
            message: "source_commit 必须是 40 位小写 hex SHA".to_string(),
        });
    }

    // 工件名唯一、SHA256 形如 64 位小写 hex。
    let mut seen = BTreeMap::new();
    for artifact in &manifest.artifacts {
        if seen.insert(artifact.name.clone(), ()).is_some() {
            violations.push(Violation {
                platform: None,
                check: None,
                message: format!("工件名重复: {}", artifact.name),
            });
        }
        if artifact.sha256.len() != 64
            || !artifact.sha256.chars().all(|c| c.is_ascii_hexdigit())
            || artifact.sha256.chars().any(|c| c.is_ascii_uppercase())
        {
            violations.push(Violation {
                platform: None,
                check: None,
                message: format!("工件 {} 的 sha256 必须是 64 位小写 hex", artifact.name),
            });
        }
    }

    // 平台覆盖:四个必验平台必须存在且恰好一次。
    let required = ["windows", "linux", "macos", "android"];
    let mut platform_count = BTreeMap::new();
    for entry in &manifest.platforms {
        *platform_count
            .entry(entry.platform.clone())
            .or_insert(0usize) += 1;
    }
    for platform in required {
        match platform_count.get(platform) {
            Some(1) => {}
            Some(n) => violations.push(Violation {
                platform: Some(platform.to_string()),
                check: None,
                message: format!("平台 {platform} 出现 {n} 次,必须恰好一次"),
            }),
            None => violations.push(Violation {
                platform: Some(platform.to_string()),
                check: None,
                message: format!("缺少必验平台 {platform}"),
            }),
        }
    }

    // 每个平台:检查键齐全、引用工件存在、frontend/os/architecture 非空。
    for entry in &manifest.platforms {
        if !required.contains(&entry.platform.as_str()) {
            violations.push(Violation {
                platform: Some(entry.platform.clone()),
                check: None,
                message: "未知平台".to_string(),
            });
        }
        for (key, state) in &entry.checks {
            if *state == CheckState::NotApplicable
                && (entry.platform != "android"
                    || key != "trainer_lifecycle"
                    || (manifest.schema_version == 2
                        && !entry
                            .exemptions
                            .get(key)
                            .is_some_and(|s| !s.trim().is_empty())))
            {
                violations.push(Violation {
                    platform: Some(entry.platform.clone()),
                    check: Some(key.clone()),
                    message:
                        "N/A 仅允许 android/trainer_lifecycle;schema 2 必须提供 exemption 原因"
                            .to_string(),
                });
            }
        }
        for key in entry.exemptions.keys() {
            if entry.checks.get(key) != Some(&CheckState::NotApplicable) {
                violations.push(Violation {
                    platform: Some(entry.platform.clone()),
                    check: Some(key.clone()),
                    message: "exemption 必须对应 N/A 项".to_string(),
                });
            }
        }
        for key in CHECK_KEYS {
            if !entry.checks.contains_key(key) {
                violations.push(Violation {
                    platform: Some(entry.platform.clone()),
                    check: Some(key.to_string()),
                    message: "缺少必查项".to_string(),
                });
            }
        }
        for key in entry.checks.keys() {
            if !CHECK_KEYS.contains(&key.as_str()) {
                violations.push(Violation {
                    platform: Some(entry.platform.clone()),
                    check: Some(key.clone()),
                    message: format!("未知检查项 {key}(允许列表见 CHECK_KEYS)"),
                });
            }
        }
        if !manifest.artifacts.iter().any(|a| a.name == entry.artifact) {
            violations.push(Violation {
                platform: Some(entry.platform.clone()),
                check: None,
                message: format!("引用工件不存在: {}", entry.artifact),
            });
        }
        if entry.frontend.trim().is_empty()
            || entry.os.trim().is_empty()
            || entry.architecture.trim().is_empty()
        {
            violations.push(Violation {
                platform: Some(entry.platform.clone()),
                check: None,
                message: "frontend/os/architecture 不能为空".to_string(),
            });
        }
    }

    violations
}

/// RC 门禁(publish=*-rc.N):结构合法 + 无 FAIL。UNVERIFIED 放行。
pub fn check_rc(manifest: &AcceptanceManifest) -> Vec<Violation> {
    let mut violations = validate_structure(manifest);
    for entry in &manifest.platforms {
        for (key, state) in &entry.checks {
            if *state == CheckState::Fail {
                violations.push(Violation {
                    platform: Some(entry.platform.clone()),
                    check: Some(key.clone()),
                    message: "FAIL 项阻塞 RC 发布".to_string(),
                });
            }
        }
    }
    violations
}

/// stable 策略校验: schema 2、同 core RC、必查 PASS、显式豁免及证据。
/// 这只是纯数据校验;发布必须调用 provenance::verify 校验独立来源和真实附件。
pub fn check_stable(manifest: &AcceptanceManifest, expected_version: &str) -> Vec<Violation> {
    check_stable_policy(manifest, expected_version, false)
}

// Only the explicit owner-authorized runtime qualification verifier may allow
// still-visible UNVERIFIED platform checks. Default/full-platform remains strict.
fn check_stable_policy(
    manifest: &AcceptanceManifest,
    expected_version: &str,
    user_platform_testing: bool,
) -> Vec<Violation> {
    let mut violations = validate_structure(manifest);
    if manifest.schema_version != 2 {
        violations.push(Violation {
            platform: None,
            check: None,
            message: "stable 必须使用 schema_version 2;历史记录不能冒充当前验收".to_string(),
        });
    }
    if !provenance::stable_version(expected_version) {
        violations.push(Violation {
            platform: None,
            check: None,
            message: "期望版本必须为规范 x.y.z stable 版本".to_string(),
        });
    }
    if !manifest
        .build_manifest_sha256
        .as_deref()
        .is_some_and(|s| provenance::is_hex(s, 64))
    {
        violations.push(Violation {
            platform: None,
            check: None,
            message: "stable 必须绑定 build_manifest_sha256".to_string(),
        });
    }

    if manifest.version != expected_version {
        violations.push(Violation {
            platform: None,
            check: None,
            message: format!(
                "清单版本 {} 与发布版本 {expected_version} 不一致",
                manifest.version
            ),
        });
    }
    match &manifest.accepted_rc {
        Some(rc) if !rc.is_empty() => {
            if provenance::rc_core(rc) != Some(expected_version) {
                violations.push(Violation {
                    platform: None,
                    check: None,
                    message: format!("accepted_rc `{rc}` 不是 RC 版本或与 stable core 不一致(须形如 {expected_version}-rc.N)"),
                });
            }
        }
        _ => violations.push(Violation {
            platform: None,
            check: None,
            message: "stable 发布必须记录 accepted_rc(依据哪个 RC 验收)".to_string(),
        }),
    }

    {
        for entry in &manifest.platforms {
            for (key, state) in &entry.checks {
                match state {
                    CheckState::Pass | CheckState::NotApplicable => {}
                    CheckState::Fail => violations.push(Violation {
                        platform: Some(entry.platform.clone()),
                        check: Some(key.clone()),
                        message: "FAIL 阻塞 stable 发布".to_string(),
                    }),
                    CheckState::Unverified if !user_platform_testing => {
                        violations.push(Violation {
                            platform: Some(entry.platform.clone()),
                            check: Some(key.clone()),
                            message: "UNVERIFIED 阻塞 stable 发布(缺少真机/等价验证)".to_string(),
                        })
                    }
                    CheckState::Unverified => {}
                }
            }
            if !entry
                .runtime
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty())
            {
                violations.push(Violation {
                    platform: Some(entry.platform.clone()),
                    check: None,
                    message: "stable 必须记录 runtime(librime/Lua/客户端版本)".to_string(),
                });
            }
            let has_pass = entry.checks.values().any(|s| *s == CheckState::Pass);
            if has_pass
                && (!entry
                    .evidence
                    .as_deref()
                    .is_some_and(|s| !s.trim().is_empty())
                    || !entry
                        .verified_at
                        .as_deref()
                        .is_some_and(provenance::utc_timestamp))
            {
                violations.push(Violation {
                    platform: Some(entry.platform.clone()),
                    check: None,
                    message: "存在 PASS 项时 evidence 非空且 verified_at 必须是有效 UTC 时间 YYYY-MM-DDTHH:MM:SSZ".to_string(),
                });
            }
        }
    }

    violations
}

/// 从清单生成 Markdown 表格;工作流用它机械比对 `docs/platform-acceptance.md`,
/// 保证文档不漂移(#148 §2)。
pub fn render_markdown(manifest: &AcceptanceManifest) -> String {
    let mut out = String::new();
    out.push_str("| 用例 | 平台 | 状态 |\n");
    out.push_str("| ---- | ---- | ---- |\n");
    const LABELS: [(&str, &str); 12] = [
        ("clean_install", "干净安装"),
        ("upgrade_from_v1", "升级安装(v1 → v2)"),
        ("schema_deployment", "方案部署"),
        ("static_smoke", "静态层冒烟"),
        ("flow_sentence_input", "Flow 连续组句"),
        ("learning_persistence", "本地学习持久化"),
        ("oov_reachability", "OOV 组合可达"),
        ("context_ranker_neutrality", "context_ranker 默认态中性"),
        ("joint_decoder_neutrality", "joint_decoder 默认态中性"),
        ("static_fallback", "xhup_flow_static 回退"),
        ("trainer_lifecycle", "Trainer 生命周期"),
        ("privacy", "隐私(无遥测/本地状态)"),
    ];
    for entry in &manifest.platforms {
        for (key, label) in LABELS {
            let state = entry
                .checks
                .get(key)
                .map(|s| s.as_str())
                .unwrap_or("UNVERIFIED");
            out.push_str(&format!("| {label} | {} | {state} |\n", entry.platform));
        }
    }
    out
}

/// 逐平台最差状态汇总(`platform=STATE`),状态严重度排序:
/// PASS < N/A < UNVERIFIED < FAIL。发布说明引用该输出,避免手写漂移。
pub fn per_platform_summary(manifest: &AcceptanceManifest) -> Vec<String> {
    fn severity(state: CheckState) -> u8 {
        match state {
            CheckState::Pass => 0,
            CheckState::NotApplicable => 1,
            CheckState::Unverified => 2,
            CheckState::Fail => 3,
        }
    }
    manifest
        .platforms
        .iter()
        .map(|entry| {
            let worst = entry
                .checks
                .values()
                .copied()
                .max_by_key(|s| (severity(*s), *s == CheckState::Fail))
                .unwrap_or(CheckState::Unverified);
            format!("{}={}", entry.platform, worst.as_str())
        })
        .collect()
}

/// 从 JSON 字符串解析清单(malformed 输入在此失败)。
pub fn parse_manifest(json: &str) -> Result<AcceptanceManifest, serde_json::Error> {
    serde_json::from_str(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_platform(platform: &str, artifact: &str) -> PlatformEntry {
        let mut checks = BTreeMap::new();
        for key in CHECK_KEYS {
            checks.insert(key.to_string(), CheckState::Unverified);
        }
        PlatformEntry {
            platform: platform.to_string(),
            frontend: "frontend".to_string(),
            os: "os".to_string(),
            architecture: "arch".to_string(),
            artifact: artifact.to_string(),
            checks,
            evidence: None,
            verified_at: None,
            runtime: Some("librime 1.16.1 / Lua 5.4 / test-client 1".to_string()),
            exemptions: BTreeMap::new(),
        }
    }

    fn base_manifest() -> AcceptanceManifest {
        let mut manifest = AcceptanceManifest {
            schema_version: 1,
            version: "2.0.0-rc.1".to_string(),
            accepted_rc: None,
            source_commit: "a".repeat(40),
            build_manifest_sha256: Some("b".repeat(64)),
            artifacts: vec![ArtifactRecord {
                name: "artifact-a".to_string(),
                sha256: "a".repeat(64),
            }],
            platforms: vec![],
        };
        for (platform, artifact) in [
            ("windows", "artifact-a"),
            ("linux", "artifact-a"),
            ("macos", "artifact-a"),
            ("android", "artifact-a"),
        ] {
            manifest.platforms.push(base_platform(platform, artifact));
        }
        manifest
    }

    #[test]
    fn valid_rc_manifest_passes_rc_gate() {
        let manifest = base_manifest();
        assert!(check_rc(&manifest).is_empty());
    }

    #[test]
    fn structure_rejects_missing_platform() {
        let mut manifest = base_manifest();
        manifest.platforms.retain(|p| p.platform != "macos");
        let violations = check_rc(&manifest);
        assert!(
            violations
                .iter()
                .any(|v| v.platform.as_deref() == Some("macos") && v.check.is_none())
        );
    }

    #[test]
    fn rc_gate_rejects_fail_but_allows_unverified() {
        let mut manifest = base_manifest();
        manifest.platforms[0]
            .checks
            .insert("clean_install".to_string(), CheckState::Fail);
        assert!(
            check_rc(&manifest)
                .iter()
                .any(|v| v.check.as_deref() == Some("clean_install"))
        );
    }

    #[test]
    fn stable_gate_rejects_unverified() {
        let mut manifest = base_manifest();
        manifest.version = "2.0.0".to_string();
        manifest.accepted_rc = Some("2.0.0-rc.1".to_string());
        // 全部 UNVERIFIED → 必须阻塞,并指明平台/检查项。
        let violations = check_stable(&manifest, "2.0.0");
        assert!(
            violations
                .iter()
                .any(|v| v.platform.as_deref() == Some("windows")
                    && v.check.as_deref() == Some("clean_install")
                    && v.message.contains("UNVERIFIED"))
        );
    }

    #[test]
    fn stable_gate_requires_accepted_rc() {
        let mut manifest = base_manifest();
        manifest.version = "2.0.0".to_string();
        for entry in &mut manifest.platforms {
            for state in entry.checks.values_mut() {
                *state = CheckState::Pass;
            }
            entry.evidence = Some("https://example.com".to_string());
            entry.verified_at = Some("2026-09-27T00:00:00Z".to_string());
        }
        let violations = check_stable(&manifest, "2.0.0");
        assert!(violations.iter().any(|v| v.message.contains("accepted_rc")));
    }

    #[test]
    fn stable_gate_rejects_version_mismatch() {
        let mut manifest = base_manifest();
        manifest.version = "2.0.1".to_string();
        manifest.accepted_rc = Some("2.0.0-rc.1".to_string());
        for entry in &mut manifest.platforms {
            for state in entry.checks.values_mut() {
                *state = CheckState::Pass;
            }
            entry.evidence = Some("e".to_string());
            entry.verified_at = Some("2026-09-27T00:00:00Z".to_string());
        }
        let violations = check_stable(&manifest, "2.0.0");
        assert!(violations.iter().any(|v| v.message.contains("不一致")));
    }

    #[test]
    fn stable_gate_passes_fully_verified() {
        let mut manifest = base_manifest();
        manifest.version = "2.0.0".to_string();
        manifest.accepted_rc = Some("2.0.0-rc.1".to_string());
        for entry in &mut manifest.platforms {
            for state in entry.checks.values_mut() {
                *state = CheckState::Pass;
            }
            entry.evidence = Some("#83 评论".to_string());
            entry.verified_at = Some("2026-09-27T00:00:00Z".to_string());
        }
        manifest.schema_version = 2;
        // Android 的 trainer_lifecycle 按显式有理由的 N/A 语义允许。
        let android = manifest
            .platforms
            .iter_mut()
            .find(|p| p.platform == "android")
            .unwrap();
        android.exemptions.insert(
            "trainer_lifecycle".to_string(),
            "无桌面控制中心".to_string(),
        );
        android
            .checks
            .insert("trainer_lifecycle".to_string(), CheckState::NotApplicable);
        assert!(check_stable(&manifest, "2.0.0").is_empty());
    }

    #[test]
    fn malformed_json_fails_parse() {
        assert!(parse_manifest("{ not json").is_err());
        // 未知状态值同样拒绝。
        assert!(parse_manifest(
            r#"{"schema_version":1,"version":"x","accepted_rc":null,"source_commit":"c","artifacts":[],"platforms":[{"platform":"windows","frontend":"f","os":"o","architecture":"a","artifact":"x","checks":{"clean_install":"MAYBE"},"evidence":null,"verified_at":null}]}"#
        )
        .is_err());
    }

    #[test]
    fn unknown_check_key_rejected() {
        let mut manifest = base_manifest();
        manifest.platforms[0]
            .checks
            .insert("mystery_check".to_string(), CheckState::Pass);
        let violations = check_rc(&manifest);
        assert!(
            violations
                .iter()
                .any(|v| v.check.as_deref() == Some("mystery_check"))
        );
    }

    #[test]
    fn stable_rc_bypass_rejected_via_wrong_accepted_rc() {
        // 用 stable=2.0.0 的清单但 accepted_rc 指向非 RC → 阻塞。
        let mut manifest = base_manifest();
        manifest.version = "2.0.0".to_string();
        manifest.accepted_rc = Some("1.9.9".to_string());
        for entry in &mut manifest.platforms {
            for state in entry.checks.values_mut() {
                *state = CheckState::Pass;
            }
            entry.evidence = Some("e".to_string());
            entry.verified_at = Some("2026-09-27T00:00:00Z".to_string());
        }
        let violations = check_stable(&manifest, "2.0.0");
        assert!(
            violations
                .iter()
                .any(|v| v.message.contains("不是 RC 版本"))
        );
    }

    #[test]
    fn pass_requires_evidence() {
        let mut manifest = base_manifest();
        manifest.accepted_rc = Some("2.0.0-rc.1".to_string());
        manifest.platforms[0]
            .checks
            .insert("clean_install".to_string(), CheckState::Pass);
        // RC gate 只关心 FAIL,不因缺证据阻塞;但 stable 会。
        assert!(check_rc(&manifest).is_empty());
        let violations = check_stable(&manifest, "2.0.0-rc.1");
        assert!(violations.iter().any(|v| v.message.contains("evidence")));
    }

    #[test]
    fn per_platform_summary_reports_worst_state() {
        let mut manifest = base_manifest();
        let windows = manifest
            .platforms
            .iter_mut()
            .find(|p| p.platform == "windows")
            .unwrap();
        windows
            .checks
            .insert("clean_install".to_string(), CheckState::Pass);
        windows
            .checks
            .insert("static_smoke".to_string(), CheckState::Fail);
        windows
            .checks
            .insert("privacy".to_string(), CheckState::NotApplicable);
        let summary = per_platform_summary(&manifest);
        let win = summary.iter().find(|l| l.starts_with("windows=")).unwrap();
        assert_eq!(win, "windows=FAIL");
        // 全 UNVERIFIED 的平台保持 UNVERIFIED。
        assert!(summary.iter().any(|l| l == "linux=UNVERIFIED"));
    }
}
