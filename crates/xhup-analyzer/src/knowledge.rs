//! Deterministic global/character evidence audits; disagreements are not deleted.
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use xhup_core::knowledge::*;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
pub struct KnowledgeFinding {
    pub category: String,
    pub character: char,
    pub details: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct KnowledgeAudit {
    pub schema: &'static str,
    pub counts: BTreeMap<String, usize>,
    pub findings: Vec<KnowledgeFinding>,
}

/// `production` is supplied independently so trusted but unsupported evidence is visible.
pub fn audit_knowledge(
    e: &KnowledgeEvidence<'_>,
    sources: &[EvidenceSource<'_>],
    core: &BTreeSet<char>,
    production: &BTreeSet<char>,
    high_frequency: u64,
) -> KnowledgeAudit {
    let mut findings = BTreeSet::new();
    let mut characters = BTreeSet::new();
    let mut trusted = BTreeSet::new();
    let mut semantic = BTreeSet::new();
    let mut values: BTreeMap<(&str, char), BTreeSet<String>> = BTreeMap::new();
    let mut counts = BTreeMap::new();
    let mut register = |kind: &'static str,
                        ch: CharacterId,
                        value: String,
                        identity: String,
                        p: Provenance<'_>| {
        characters.insert(ch.0);
        *counts.entry(format!("{kind}_evidence")).or_default() += 1;
        values.entry((kind, ch.0)).or_default().insert(value);
        if !semantic.insert((kind, ch, identity, p.source.to_string())) {
            findings.insert(KnowledgeFinding {
                category: "duplicate_evidence".into(),
                character: ch.0,
                details: format!("{kind}: {}", p.source),
            });
        }
        if let Err(err) = validate_provenance(p, sources) {
            findings.insert(KnowledgeFinding {
                category: "invalid_provenance".into(),
                character: ch.0,
                details: err.to_string(),
            });
        } else if p.status.is_active() && p.confidence != EvidenceConfidence::Low {
            trusted.insert(ch.0);
        }
    };
    for r in &e.readings {
        register(
            "reading",
            r.character,
            r.reading.into(),
            r.reading.into(),
            r.provenance,
        );
    }
    for r in &e.sounds {
        register(
            "sound",
            r.character,
            r.code.to_string(),
            r.code.to_string(),
            r.provenance,
        );
    }
    for r in &e.shapes {
        register(
            "shape",
            r.character,
            r.code.to_string(),
            r.code.to_string(),
            r.provenance,
        );
    }
    for r in &e.full_codes {
        register(
            "full",
            r.character,
            r.code().to_string(),
            r.code().to_string(),
            r.provenance,
        );
    }
    for r in &e.generated {
        register(
            "generated",
            r.character,
            r.code.to_string(),
            format!("{}:{}", r.code, r.reading),
            r.provenance,
        );
    }
    for r in &e.frequencies {
        register(
            "frequency",
            r.character,
            r.value.to_string(),
            format!("{:?}:{:?}", r.kind, r.reading),
            r.provenance,
        );
    }
    for r in &e.variants {
        register(
            "variant",
            r.character,
            r.target.0.to_string(),
            format!("{}:{:?}", r.target.0, r.kind),
            r.provenance,
        );
    }
    for kind in [
        "reading",
        "sound",
        "shape",
        "full",
        "generated",
        "frequency",
        "variant",
    ] {
        counts.entry(format!("{kind}_evidence")).or_default();
    }
    for (&(kind, ch), codes) in &values {
        if matches!(kind, "sound" | "shape" | "full") && codes.len() > 1 {
            findings.insert(KnowledgeFinding {
                category: format!("conflicting_{kind}"),
                character: ch,
                details: codes.iter().cloned().collect::<Vec<_>>().join(","),
            });
        }
    }
    for &ch in &characters {
        let sound = values.contains_key(&("sound", ch)) || values.contains_key(&("full", ch));
        let shape = values.contains_key(&("shape", ch)) || values.contains_key(&("full", ch));
        for (condition, category) in [
            (!core.contains(&ch), "outside_core"),
            (
                trusted.contains(&ch) && !production.contains(&ch),
                "unsupported_trusted",
            ),
            (sound && !shape, "sound_without_shape"),
            (shape && !sound, "shape_without_sound"),
        ] {
            if condition {
                findings.insert(KnowledgeFinding {
                    category: category.into(),
                    character: ch,
                    details: String::new(),
                });
            }
        }
    }
    for r in &e.frequencies {
        if r.kind == FrequencyKind::ReadingScore
            && r.value >= high_frequency
            && r.provenance.status.is_active()
            && r.provenance.confidence != EvidenceConfidence::Low
            && validate_provenance(r.provenance, sources).is_ok()
            && !production.contains(&r.character.0)
        {
            findings.insert(KnowledgeFinding {
                category: "high_frequency_without_input".into(),
                character: r.character.0,
                details: format!("{}:{}", r.provenance.source, r.value),
            });
        }
    }
    for category in [
        "conflicting_sound",
        "conflicting_shape",
        "conflicting_full",
        "outside_core",
        "unsupported_trusted",
        "sound_without_shape",
        "shape_without_sound",
        "duplicate_evidence",
        "invalid_provenance",
        "high_frequency_without_input",
    ] {
        counts.insert(
            category.into(),
            findings.iter().filter(|f| f.category == category).count(),
        );
    }
    counts.insert("core_characters".into(), core.len());
    counts.insert("input_characters".into(), production.len());
    counts.insert("characters_with_evidence".into(), characters.len());
    counts.insert(
        "distinct_sources".into(),
        sources.iter().map(|s| s.id).collect::<BTreeSet<_>>().len(),
    );
    let resolved = resolve_full_codes(&e.full_codes, sources);
    counts.insert(
        "resolved_attested_full_relations".into(),
        resolved.as_ref().map_or(0, Vec::len),
    );
    let mut all_codes: BTreeSet<_> = resolved
        .into_iter()
        .flatten()
        .map(|r| (r.character, r.code))
        .collect();
    all_codes.extend(
        e.generated
            .iter()
            .filter(|r| {
                r.provenance.status.is_active()
                    && r.provenance.confidence != EvidenceConfidence::Low
                    && validate_provenance(r.provenance, sources).is_ok()
            })
            .map(|r| (r.character, r.code)),
    );
    counts.insert("resolved_full_relations".into(), all_codes.len());
    KnowledgeAudit {
        schema: "xhup-knowledge-audit/v1",
        counts,
        findings: findings.into_iter().collect(),
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CharacterExplanation {
    pub character: char,
    pub core_standard: bool,
    pub input_member: bool,
    pub normalized_evidence: String,
    pub resolved_full_codes: Vec<String>,
    pub preferred_sources: BTreeMap<String, String>,
    pub sources_tsv: String,
    pub findings: Vec<KnowledgeFinding>,
}
pub fn explain_character(
    ch: char,
    e: &KnowledgeEvidence<'_>,
    sources: &[EvidenceSource<'_>],
    core: &BTreeSet<char>,
    production: &BTreeSet<char>,
) -> Result<CharacterExplanation, EvidenceError> {
    let mut subset = e.clone();
    subset.readings.retain(|r| r.character.0 == ch);
    subset.sounds.retain(|r| r.character.0 == ch);
    subset.shapes.retain(|r| r.character.0 == ch);
    subset.full_codes.retain(|r| r.character.0 == ch);
    subset.generated.retain(|r| r.character.0 == ch);
    subset.frequencies.retain(|r| r.character.0 == ch);
    subset.variants.retain(|r| r.character.0 == ch);
    let normalized_evidence = serialize_evidence(&subset)?;
    let source_ids: BTreeSet<_> = normalized_evidence
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split('\t').nth(4))
        .collect();
    let used_sources: Vec<_> = sources
        .iter()
        .filter(|s| source_ids.contains(s.id))
        .cloned()
        .collect();
    let resolved = resolve_full_codes(&subset.full_codes, sources)?;
    let preferred_sources = resolved
        .iter()
        .map(|r| (r.code.to_string(), r.preferred.source.to_string()))
        .collect();
    let mut codes: BTreeSet<_> = resolved.iter().map(|r| r.code.to_string()).collect();
    codes.extend(
        subset
            .generated
            .iter()
            .filter(|r| {
                r.provenance.status.is_active()
                    && r.provenance.confidence != EvidenceConfidence::Low
                    && validate_provenance(r.provenance, sources).is_ok()
            })
            .map(|r| r.code.to_string()),
    );
    let report = audit_knowledge(&subset, sources, core, production, 100_000);
    Ok(CharacterExplanation {
        character: ch,
        core_standard: core.contains(&ch),
        input_member: production.contains(&ch),
        normalized_evidence,
        resolved_full_codes: codes.into_iter().collect(),
        preferred_sources,
        sources_tsv: serialize_sources(&used_sources)?,
        findings: report.findings,
    })
}
