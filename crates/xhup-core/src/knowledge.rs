//! Validated, source-preserving character evidence. No candidate ranking or I/O.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use crate::{DoublePinyinCode, FullCode, ShapeCode};

mod normalized;
mod source;
pub use normalized::{parse_evidence, serialize_evidence};
pub use source::{
    EvidenceSource, SourceKind, SourceUse, canonical_sources, parse_sources, serialize_sources,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceError(pub String);
impl fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for EvidenceError {}
pub(super) fn invalid(message: impl Into<String>) -> EvidenceError {
    EvidenceError(message.into())
}

/// Unicode identity, deliberately independent of either curated membership set.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CharacterId(pub char);
impl FromStr for CharacterId {
    type Err = EvidenceError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut chars = text.chars();
        match (chars.next(), chars.next()) {
            (Some(ch), None) if !ch.is_control() && !ch.is_whitespace() => Ok(Self(ch)),
            _ => Err(invalid(
                "character must be one non-whitespace Unicode scalar",
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum EvidenceStatus {
    Official,
    OfficialYieldFull,
    OfficialOutsideCore,
    Attested,
    LegacyCompatible,
    Generated,
    Alternate,
    Deprecated,
    Uncertain,
}
impl EvidenceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Official => "official",
            Self::OfficialYieldFull => "official-yield-full",
            Self::OfficialOutsideCore => "official-outside-core",
            Self::Attested => "attested",
            Self::LegacyCompatible => "legacy-compatible",
            Self::Generated => "generated",
            Self::Alternate => "alternate",
            Self::Deprecated => "deprecated",
            Self::Uncertain => "uncertain",
        }
    }
    pub fn is_active(self) -> bool {
        !matches!(self, Self::Deprecated | Self::Uncertain)
    }
    pub fn priority(self) -> u8 {
        match self {
            Self::Official | Self::OfficialYieldFull | Self::OfficialOutsideCore => 6,
            Self::LegacyCompatible => 5,
            Self::Attested => 4,
            Self::Alternate => 3,
            Self::Generated => 2,
            Self::Uncertain => 1,
            Self::Deprecated => 0,
        }
    }
}
impl FromStr for EvidenceStatus {
    type Err = EvidenceError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        [
            Self::Official,
            Self::OfficialYieldFull,
            Self::OfficialOutsideCore,
            Self::Attested,
            Self::LegacyCompatible,
            Self::Generated,
            Self::Alternate,
            Self::Deprecated,
            Self::Uncertain,
        ]
        .into_iter()
        .find(|status| status.as_str() == s)
        .ok_or_else(|| invalid(format!("unknown evidence status: {s}")))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum EvidenceConfidence {
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Provenance<'a> {
    pub source: &'a str,
    pub status: EvidenceStatus,
    pub confidence: EvidenceConfidence,
}

/// A reading is a normalized linguistic spelling, never inferred from sound keys.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ReadingEvidence<'a> {
    pub character: CharacterId,
    pub reading: &'a str,
    pub primary: bool,
    pub provenance: Provenance<'a>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SoundCodeEvidence<'a> {
    pub character: CharacterId,
    pub code: DoublePinyinCode,
    pub provenance: Provenance<'a>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ShapeCodeEvidence<'a> {
    pub character: CharacterId,
    pub code: ShapeCode,
    pub provenance: Provenance<'a>,
}
/// Sound/shape pairing is preserved; independent projections never imply a cross product.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct FullCodeEvidence<'a> {
    pub character: CharacterId,
    pub sound: DoublePinyinCode,
    pub shape: ShapeCode,
    pub weight: u32,
    pub provenance: Provenance<'a>,
}
impl FullCodeEvidence<'_> {
    pub fn code(self) -> FullCode {
        FullCode::from_parts(self.sound, self.shape)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct GeneratedCodeEvidence<'a> {
    pub character: CharacterId,
    pub code: FullCode,
    pub reading: &'a str,
    pub provenance: Provenance<'a>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum FrequencyKind {
    ReadingScore,
    InputWeight,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct FrequencyEvidence<'a> {
    pub character: CharacterId,
    pub reading: Option<&'a str>,
    pub value: u64,
    pub kind: FrequencyKind,
    pub provenance: Provenance<'a>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum VariantKind {
    Simplified,
    Traditional,
    Compatibility,
    Other,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct VariantEvidence<'a> {
    pub character: CharacterId,
    pub target: CharacterId,
    pub kind: VariantKind,
    pub provenance: Provenance<'a>,
}

/// Normalized relations are separate collections, not an untyped property graph.
#[derive(Clone, Debug, Default)]
pub struct KnowledgeEvidence<'a> {
    pub readings: Vec<ReadingEvidence<'a>>,
    pub sounds: Vec<SoundCodeEvidence<'a>>,
    pub shapes: Vec<ShapeCodeEvidence<'a>>,
    pub full_codes: Vec<FullCodeEvidence<'a>>,
    pub generated: Vec<GeneratedCodeEvidence<'a>>,
    pub frequencies: Vec<FrequencyEvidence<'a>>,
    pub variants: Vec<VariantEvidence<'a>>,
}

pub(super) fn rows(text: &str, columns: usize) -> Result<Vec<(usize, Vec<&str>)>, EvidenceError> {
    if !text.ends_with('\n') || text.contains('\r') || text.starts_with('\u{feff}') {
        return Err(invalid(
            "expected UTF-8 without BOM, LF lines and a final newline",
        ));
    }
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.starts_with('#'))
        .map(|(i, line)| {
            let fields: Vec<_> = line.split('\t').collect();
            if fields.len() != columns || fields.iter().any(|v| v.is_empty() || v.trim() != *v) {
                Err(invalid(format!(
                    "row {}: expected {columns} nonempty TAB fields",
                    i + 1
                )))
            } else {
                Ok((i + 1, fields))
            }
        })
        .collect()
}

/// Compatibility adapter for the existing six-column source table; no snapshot ceiling.
pub fn parse_full_codes(text: &str) -> Result<Vec<FullCodeEvidence<'_>>, EvidenceError> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    let mut previous = None;
    for (row, f) in rows(text, 6)? {
        let character = f[0].parse()?;
        let sound = f[1]
            .parse()
            .map_err(|e| invalid(format!("row {row}: {e}")))?;
        let shape = f[2]
            .parse()
            .map_err(|e| invalid(format!("row {row}: {e}")))?;
        let weight = decimal(f[3])?
            .try_into()
            .map_err(|_| invalid(format!("row {row}: u32 overflow")))?;
        let status = f[5].parse()?;
        let key = (character, sound, shape, f[4], f[5]);
        if previous.is_some_and(|p| p >= key) || !seen.insert((character, sound, shape, f[4])) {
            return Err(invalid(format!(
                "row {row}: duplicate semantic evidence or unsorted row"
            )));
        }
        previous = Some(key);
        result.push(FullCodeEvidence {
            character,
            sound,
            shape,
            weight,
            provenance: Provenance {
                source: f[4],
                status,
                confidence: EvidenceConfidence::High,
            },
        });
    }
    Ok(result)
}

pub fn serialize_full_codes(evidence: &[FullCodeEvidence<'_>]) -> Result<String, EvidenceError> {
    let mut sorted = evidence.to_vec();
    sorted.sort_by_key(|e| {
        (
            e.character,
            e.sound,
            e.shape,
            e.provenance.source,
            e.provenance.status.as_str(),
        )
    });
    let mut text = String::new();
    for e in sorted {
        if e.provenance.confidence != EvidenceConfidence::High {
            return Err(invalid("legacy TSV only encodes high-confidence evidence"));
        }
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\n",
            e.character.0,
            e.sound,
            e.shape,
            e.weight,
            e.provenance.source,
            e.provenance.status.as_str()
        ));
    }
    if !text.is_empty() {
        parse_full_codes(&text)?;
    }
    Ok(text)
}

pub fn decimal(s: &str) -> Result<u64, EvidenceError> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) || (s.len() > 1 && s.starts_with('0'))
    {
        return Err(invalid("expected canonical unsigned decimal"));
    }
    s.parse().map_err(|_| invalid("unsigned decimal overflow"))
}

/// Production union retains all accepted aliases. Preferred evidence explains a relation;
/// it does not suppress other codes or change static candidate ordering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFullCode<'a> {
    pub character: CharacterId,
    pub code: FullCode,
    pub preferred: Provenance<'a>,
    pub supporting: Vec<Provenance<'a>>,
}

pub fn validate_provenance(
    p: Provenance<'_>,
    sources: &[EvidenceSource<'_>],
) -> Result<(), EvidenceError> {
    let source = sources
        .iter()
        .find(|s| s.id == p.source)
        .ok_or_else(|| invalid(format!("orphan source: {}", p.source)))?;
    if matches!(
        p.status,
        EvidenceStatus::Official
            | EvidenceStatus::OfficialYieldFull
            | EvidenceStatus::OfficialOutsideCore
    ) && source.kind != SourceKind::Oracle
    {
        return Err(invalid(
            "official status requires an official oracle source",
        ));
    }
    if p.status == EvidenceStatus::Generated && source.kind != SourceKind::Project {
        return Err(invalid(
            "generated status requires a versioned project derivation",
        ));
    }
    if source.usage == SourceUse::Oracle
        && !matches!(
            p.status,
            EvidenceStatus::Official
                | EvidenceStatus::OfficialYieldFull
                | EvidenceStatus::OfficialOutsideCore
                | EvidenceStatus::Uncertain
                | EvidenceStatus::Deprecated
        )
    {
        return Err(invalid(
            "oracle source cannot be relabelled as redistributable evidence",
        ));
    }
    Ok(())
}

pub fn resolve_full_codes<'a>(
    evidence: &[FullCodeEvidence<'a>],
    sources: &[EvidenceSource<'_>],
) -> Result<Vec<ResolvedFullCode<'a>>, EvidenceError> {
    let mut seen = BTreeSet::new();
    let mut groups: BTreeMap<_, Vec<Provenance<'a>>> = BTreeMap::new();
    for e in evidence {
        validate_provenance(e.provenance, sources)?;
        if !seen.insert((e.character, e.code(), e.provenance.source)) {
            return Err(invalid("duplicate full-code semantic evidence"));
        }
        if e.provenance.status.is_active() && e.provenance.confidence != EvidenceConfidence::Low {
            groups
                .entry((e.character, e.code()))
                .or_default()
                .push(e.provenance);
        }
    }
    Ok(groups
        .into_iter()
        .map(|((character, code), mut supporting)| {
            supporting.sort_by(|a, b| {
                let priority = |p: &Provenance<'_>| {
                    sources.iter().find(|s| s.id == p.source).unwrap().priority
                };
                b.status
                    .priority()
                    .cmp(&a.status.priority())
                    .then(b.confidence.cmp(&a.confidence))
                    .then(priority(b).cmp(&priority(a)))
                    .then(a.source.cmp(b.source))
                    .then(a.status.cmp(&b.status))
            });
            ResolvedFullCode {
                character,
                code,
                preferred: supporting[0],
                supporting,
            }
        })
        .collect())
}
