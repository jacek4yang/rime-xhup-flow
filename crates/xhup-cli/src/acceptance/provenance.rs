//! Small exact-byte RC promotion contract. No rebuild/equivalence shortcut.
use super::{AcceptanceManifest, ArtifactRecord, Violation, check_stable};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

pub const BUILD_MANIFEST: &str = "BUILD-MANIFEST.json";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildManifest {
    pub schema_version: u32,
    pub version: String,
    pub source_commit: String,
    /// Includes the existing BUILD-INFO and canonical/file checksum manifests.
    pub artifacts: Vec<ArtifactRecord>,
}

pub fn is_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn number(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|b| b.is_ascii_digit())
        && (value.len() == 1 || !value.starts_with('0'))
}

pub fn stable_version(value: &str) -> bool {
    let parts: Vec<_> = value.split('.').collect();
    parts.len() == 3 && parts.iter().all(|s| number(s))
}

pub fn rc_core(value: &str) -> Option<&str> {
    let (core, n) = value.split_once("-rc.")?;
    (stable_version(core) && number(n) && n != "0").then_some(core)
}

/// Deliberately one documented timestamp representation, including calendar validity.
pub fn utc_timestamp(value: &str) -> bool {
    let b = value.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return false;
    }
    let part = |start: usize, end: usize| -> Option<u32> {
        let bytes = &b[start..end];
        bytes
            .iter()
            .all(u8::is_ascii_digit)
            .then(|| bytes.iter().fold(0, |n, c| n * 10 + u32::from(c - b'0')))
    };
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        part(0, 4),
        part(5, 7),
        part(8, 10),
        part(11, 13),
        part(14, 16),
        part(17, 19),
    ) else {
        return false;
    };
    if year == 0 || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    (1..=days[month as usize - 1]).contains(&day) && hour < 24 && minute < 60 && second < 60
}

fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn file_digest(path: &Path) -> io::Result<String> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.len() == 0 {
        return Err(io::Error::other(
            "artifact must be a nonempty regular file (not symlink)",
        ));
    }
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

/// Expected published payload: all eight signed-RC packages plus three metadata files.
/// Rehearsal unsigned APKs deliberately cannot satisfy stable promotion.
pub fn payload_names(version: &str) -> Vec<String> {
    let mut names = vec![format!("xhup-flow-rime-v{version}.zip")];
    for suffix in [
        "windows-x64-setup.exe",
        "windows-x64.msi",
        "macos-universal.dmg",
        "linux-amd64.deb",
        "linux-x86_64.rpm",
        "android-arm64.apk",
        "android-universal.apk",
    ] {
        names.push(format!("xhup-flow-trainer-v{version}-{suffix}"));
    }
    names.extend(
        [
            "SHA256SUMS.txt",
            "CANONICAL-SHA256SUMS.txt",
            "BUILD-INFO.txt",
        ]
        .map(str::to_string),
    );
    names.sort();
    names
}

/// Seal the published RC payload. This command records bytes, NOT hardware acceptance.
/// Existing output is never silently overwritten.
pub fn seal(version: &str, source: &str, directory: &Path) -> io::Result<()> {
    if rc_core(version).is_none() || !is_hex(source, 40) {
        return Err(io::Error::other(
            "seal requires canonical RC version and exact source SHA",
        ));
    }
    let artifacts = payload_names(version)
        .into_iter()
        .map(|name| {
            let sha256 = file_digest(&directory.join(&name))?;
            Ok(ArtifactRecord { name, sha256 })
        })
        .collect::<io::Result<Vec<_>>>()?;
    let build = BuildManifest {
        schema_version: 1,
        version: version.to_string(),
        source_commit: source.to_string(),
        artifacts,
    };
    let mut bytes = serde_json::to_vec_pretty(&build)?;
    bytes.push(b'\n');
    // Validate the entire directory before recording provenance.
    let mut expected = payload_names(version);
    let mut actual = fs::read_dir(directory)?
        .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<io::Result<Vec<_>>>()?;
    expected.sort();
    actual.sort();
    if expected != actual {
        return Err(io::Error::other("unexpected or missing RC payload files"));
    }
    use std::io::Write;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join(BUILD_MANIFEST))?;
    output.write_all(&bytes)?;
    output.sync_all()
}

/// Verify against an independently resolved RC tag commit and actual release bytes.
/// The workflow supplies source from GitHub's commit resolution, never from this JSON.
pub fn verify(
    manifest: &AcceptanceManifest,
    version: &str,
    expected_source: &str,
    directory: &Path,
) -> Vec<Violation> {
    let mut violations = check_stable(manifest, version);
    let mut fail = |message: String| {
        violations.push(Violation {
            platform: None,
            check: None,
            message,
        })
    };
    if !is_hex(expected_source, 40) || manifest.source_commit != expected_source {
        fail("source_commit differs from independently resolved accepted RC source".to_string());
    }
    let raw = match fs::read(directory.join(BUILD_MANIFEST)) {
        Ok(raw) => raw,
        Err(e) => {
            fail(format!("cannot read {BUILD_MANIFEST}: {e}"));
            return violations;
        }
    };
    if manifest.build_manifest_sha256.as_deref() != Some(digest(&raw).as_str()) {
        fail("build_manifest_sha256 mismatch".to_string());
    }
    let build: BuildManifest = match serde_json::from_slice(&raw) {
        Ok(build) => build,
        Err(e) => {
            fail(format!("invalid build manifest: {e}"));
            return violations;
        }
    };
    if build.schema_version != 1
        || Some(build.version.as_str()) != manifest.accepted_rc.as_deref()
        || build.source_commit != expected_source
    {
        fail("build manifest schema/version/source does not match accepted RC".to_string());
    }
    let records = |items: &[ArtifactRecord]| -> BTreeMap<String, String> {
        items
            .iter()
            .map(|a| (a.name.clone(), a.sha256.clone()))
            .collect()
    };
    let accepted = records(&manifest.artifacts);
    let built = records(&build.artifacts);
    if built != accepted || built.len() != build.artifacts.len() {
        fail("accepted artifact set/digests differ from build manifest".to_string());
    }
    let names: Vec<_> = built.keys().cloned().collect();
    if names != payload_names(&build.version) {
        fail("build manifest must contain exact complete published RC payload".to_string());
    }
    for (name, expected) in &built {
        if !safe_name(name) || !is_hex(expected, 64) {
            fail("unsafe artifact name or malformed digest".to_string());
            continue;
        }
        match file_digest(&directory.join(name)) {
            Ok(actual) if &actual == expected => {}
            Ok(_) => fail(format!("artifact digest mismatch: {name}")),
            Err(e) => fail(format!("cannot verify artifact {name}: {e}")),
        }
    }
    // Also reject unlisted extra assets, directories and symlink manifest.
    let mut expected = names;
    expected.push(BUILD_MANIFEST.to_string());
    expected.sort();
    let actual = fs::read_dir(directory).and_then(|entries| {
        let mut names = entries
            .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
            .collect::<io::Result<Vec<_>>>()?;
        names.sort();
        Ok(names)
    });
    if actual.as_ref().ok() != Some(&expected)
        || fs::symlink_metadata(directory.join(BUILD_MANIFEST)).map_or(true, |m| !m.is_file())
    {
        fail("unexpected assets or non-regular build manifest".to_string());
    }
    violations
}
