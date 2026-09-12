//! Offline adapters over existing canonical tables; no duplicate checked-in dataset.
use xhup_core::XhupHanzi;
use xhup_core::knowledge::*;

fn provenance(source: &'static str, status: EvidenceStatus) -> Provenance<'static> {
    Provenance {
        source,
        status,
        confidence: EvidenceConfidence::High,
    }
}

/// Materialize the build-time evidence view. No InputHanzi dependency/initialization cycle.
pub fn canonical_knowledge() -> Result<KnowledgeEvidence<'static>, EvidenceError> {
    let mut evidence = KnowledgeEvidence::default();
    for core in XhupHanzi::all() {
        let character = CharacterId(core.as_char());
        for &reading in core.readings() {
            evidence.readings.push(ReadingEvidence {
                character,
                reading: reading.as_str(),
                primary: reading == core.primary_reading(),
                provenance: provenance("core-readings", EvidenceStatus::Attested),
            });
            if let Some(syllable) = reading.to_input_syllable() {
                let sound = syllable.to_double_pinyin_code();
                evidence.sounds.push(SoundCodeEvidence {
                    character,
                    code: sound,
                    provenance: provenance("flow-core-composition", EvidenceStatus::Generated),
                });
                for &shape in core.shape_codes() {
                    evidence.generated.push(GeneratedCodeEvidence {
                        character,
                        code: xhup_core::FullCode::from_parts(sound, shape),
                        reading: reading.as_str(),
                        provenance: provenance("flow-core-composition", EvidenceStatus::Generated),
                    });
                }
            }
        }
        // Core shape rows are the same historical source projection, not corroboration.
        for &shape in core.shape_codes() {
            evidence.shapes.push(ShapeCodeEvidence {
                character,
                code: shape,
                provenance: provenance("rime-fast-xhup", EvidenceStatus::LegacyCompatible),
            });
        }
    }
    evidence.full_codes =
        parse_full_codes(include_str!("../../../data/xhup/attested_char_codes.tsv"))?;
    for full in &evidence.full_codes {
        evidence.sounds.push(SoundCodeEvidence {
            character: full.character,
            code: full.sound,
            provenance: full.provenance,
        });
        evidence.shapes.push(ShapeCodeEvidence {
            character: full.character,
            code: full.shape,
            provenance: full.provenance,
        });
        // Source weights remain paired with full codes; they are not linguistic frequency.
    }
    // Projection duplicates are expected and removed here, before normalized validation.
    evidence.sounds.sort_by_key(|r| {
        (
            r.character,
            r.code,
            r.provenance.source,
            std::cmp::Reverse(r.provenance.status.priority()),
            r.provenance.status,
        )
    });
    evidence
        .sounds
        .dedup_by_key(|r| (r.character, r.code, r.provenance.source));
    evidence.shapes.sort_by_key(|r| {
        (
            r.character,
            r.code,
            r.provenance.source,
            std::cmp::Reverse(r.provenance.status.priority()),
            r.provenance.status,
        )
    });
    evidence
        .shapes
        .dedup_by_key(|r| (r.character, r.code, r.provenance.source));
    for (index, line) in include_str!("../../../data/frequency/wanxiang_reading_scores.tsv")
        .lines()
        .enumerate()
    {
        if line.starts_with('#') {
            continue;
        }
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 3 {
            return Err(EvidenceError(format!(
                "frequency row {}: expected three fields",
                index + 1
            )));
        }
        evidence.frequencies.push(FrequencyEvidence {
            character: fields[0].parse()?,
            reading: Some(fields[1]),
            value: decimal(fields[2])?,
            kind: FrequencyKind::ReadingScore,
            provenance: provenance("wanxiang-characters", EvidenceStatus::Attested),
        });
    }
    Ok(evidence)
}
