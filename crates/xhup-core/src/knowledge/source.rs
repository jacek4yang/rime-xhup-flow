use super::{EvidenceError, decimal, invalid, rows};
use std::collections::BTreeSet;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceKind {
    Reading,
    Xhup,
    Oracle,
    Frequency,
    Lexicon,
    Project,
}
impl SourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reading => "reading",
            Self::Xhup => "xhup",
            Self::Oracle => "oracle",
            Self::Frequency => "frequency",
            Self::Lexicon => "lexicon",
            Self::Project => "project",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceUse {
    Redistributable,
    Oracle,
}
impl SourceUse {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Redistributable => "redistributable",
            Self::Oracle => "oracle",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceSource<'a> {
    pub id: &'a str,
    pub kind: SourceKind,
    pub url: &'a str,
    pub revision: &'a str,
    pub path: &'a str,
    pub blob: Option<&'a str>,
    pub sha256: Option<&'a str>,
    pub license: &'a str,
    pub extractor: &'a str,
    pub usage: SourceUse,
    pub priority: u16,
    pub notes: &'a str,
}

pub fn canonical_sources() -> &'static [EvidenceSource<'static>] {
    static SOURCES: OnceLock<Vec<EvidenceSource<'static>>> = OnceLock::new();
    SOURCES.get_or_init(|| {
        parse_sources(include_str!("../../../../data/xhup/sources.tsv"))
            .expect("valid source registry")
    })
}

pub fn parse_sources(text: &str) -> Result<Vec<EvidenceSource<'_>>, EvidenceError> {
    if !text.starts_with("# xhup-knowledge-sources/v1\n") {
        return Err(invalid("expected xhup-knowledge-sources/v1"));
    }
    let mut result = Vec::new();
    let mut ids = BTreeSet::new();
    for (row, f) in rows(text, 12)? {
        if !f[0]
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || !ids.insert(f[0])
        {
            return Err(invalid(format!(
                "row {row}: invalid or duplicate source id"
            )));
        }
        if result
            .last()
            .is_some_and(|s: &EvidenceSource<'_>| s.id >= f[0])
        {
            return Err(invalid("sources must be sorted by id"));
        }
        let kind = [
            SourceKind::Reading,
            SourceKind::Xhup,
            SourceKind::Oracle,
            SourceKind::Frequency,
            SourceKind::Lexicon,
            SourceKind::Project,
        ]
        .into_iter()
        .find(|k| k.as_str() == f[1])
        .ok_or_else(|| invalid("unknown source kind"))?;
        let usage = match f[9] {
            "redistributable" => SourceUse::Redistributable,
            "oracle" => SourceUse::Oracle,
            _ => return Err(invalid("unknown source use")),
        };
        if !f[2].starts_with("https://")
            || f[2].contains(char::is_whitespace)
            || f[3] == "-"
            || matches!(f[3], "main" | "master" | "latest" | "HEAD")
        {
            return Err(invalid("source requires HTTPS URL and pinned revision"));
        }
        if f[4].starts_with('/')
            || f[4].contains(['\\', ':'])
            || f[4].split('/').any(|part| matches!(part, ".." | "." | ""))
        {
            return Err(invalid("source path must be upstream-relative"));
        }
        for (value, len) in [(f[5], 40), (f[6], 64)] {
            if value != "-"
                && (value.len() != len
                    || !value
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
            {
                return Err(invalid("invalid source content hash"));
            }
        }
        if !matches!(
            f[7],
            "MIT"
                | "LGPL-3.0"
                | "LGPL-3.0-only"
                | "CC-BY-4.0"
                | "Unicode-3.0"
                | "GPL-3.0-only"
                | "oracle-facts-only"
        ) || (usage == SourceUse::Redistributable && f[7] == "oracle-facts-only")
            || (usage == SourceUse::Oracle && f[7] != "oracle-facts-only")
        {
            return Err(invalid("source license/use is not reviewed"));
        }
        if f[8] == "-" {
            return Err(invalid("extractor version is required"));
        }
        result.push(EvidenceSource {
            id: f[0],
            kind,
            url: f[2],
            revision: f[3],
            path: f[4],
            blob: (f[5] != "-").then_some(f[5]),
            sha256: (f[6] != "-").then_some(f[6]),
            license: f[7],
            extractor: f[8],
            usage,
            priority: decimal(f[10])?
                .try_into()
                .map_err(|_| invalid("source priority exceeds u16"))?,
            notes: f[11],
        });
    }
    Ok(result)
}

pub fn serialize_sources(sources: &[EvidenceSource<'_>]) -> Result<String, EvidenceError> {
    let mut sorted = sources.to_vec();
    sorted.sort_by_key(|s| s.id);
    let mut text = String::from("# xhup-knowledge-sources/v1\n");
    for s in sorted {
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            s.id,
            s.kind.as_str(),
            s.url,
            s.revision,
            s.path,
            s.blob.unwrap_or("-"),
            s.sha256.unwrap_or("-"),
            s.license,
            s.extractor,
            s.usage.as_str(),
            s.priority,
            s.notes
        ));
    }
    if !text.is_empty() {
        parse_sources(&text)?;
    }
    Ok(text)
}
