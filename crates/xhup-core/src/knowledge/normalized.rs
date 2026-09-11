use super::*;

const HEADER: &str = "# xhup-knowledge-evidence/v1\n";

/// Seven-column interchange format. Source existence is validated at resolution/audit time.
pub fn parse_evidence(text: &str) -> Result<KnowledgeEvidence<'_>, EvidenceError> {
    if !text.starts_with(HEADER) {
        return Err(invalid("expected xhup-knowledge-evidence/v1"));
    }
    let mut evidence = KnowledgeEvidence::default();
    let mut previous = None;
    let mut semantic = BTreeSet::new();
    for (row, f) in rows(text, 7)? {
        let key = (f[0], f[1], f[2], f[3], f[4], f[5], f[6]);
        if previous.is_some_and(|p| p >= key) {
            return Err(invalid(format!(
                "row {row}: unsorted or duplicate evidence"
            )));
        }
        previous = Some(key);
        let detail_key = if matches!(f[0], "reading" | "full") {
            ""
        } else {
            f[3]
        };
        let value_key = if f[0] == "frequency" { "" } else { f[2] };
        if !semantic.insert((f[0], f[1], value_key, detail_key, f[4])) {
            return Err(invalid(format!("row {row}: duplicate semantic evidence")));
        }
        let character = f[1].parse()?;
        let provenance = Provenance {
            source: f[4],
            status: f[5].parse()?,
            confidence: match f[6] {
                "high" => EvidenceConfidence::High,
                "medium" => EvidenceConfidence::Medium,
                "low" => EvidenceConfidence::Low,
                _ => return Err(invalid("unknown confidence")),
            },
        };
        let reading = |value: &str| -> Result<(), EvidenceError> {
            if value.is_empty() || !value.bytes().all(|b| b.is_ascii_lowercase()) {
                Err(invalid("reading must be normalized lowercase ASCII"))
            } else {
                Ok(())
            }
        };
        let code_error = |e: crate::CodeError| invalid(e.to_string());
        match f[0] {
            "reading" => {
                reading(f[2])?;
                let primary = match f[3] {
                    "primary" => true,
                    "alt" => false,
                    _ => return Err(invalid("reading role must be primary or alt")),
                };
                evidence.readings.push(ReadingEvidence {
                    character,
                    reading: f[2],
                    primary,
                    provenance,
                });
            }
            "sound" if f[3] == "-" => evidence.sounds.push(SoundCodeEvidence {
                character,
                code: f[2].parse().map_err(code_error)?,
                provenance,
            }),
            "shape" if f[3] == "-" => evidence.shapes.push(ShapeCodeEvidence {
                character,
                code: f[2].parse().map_err(code_error)?,
                provenance,
            }),
            "full" => {
                let code: FullCode = f[2].parse().map_err(code_error)?;
                let code_text = code.to_string();
                evidence.full_codes.push(FullCodeEvidence {
                    character,
                    sound: code_text[..2].parse().map_err(code_error)?,
                    shape: code_text[2..].parse().map_err(code_error)?,
                    weight: decimal(f[3])?
                        .try_into()
                        .map_err(|_| invalid("full code weight overflow"))?,
                    provenance,
                });
            }
            "generated" => {
                reading(f[3])?;
                evidence.generated.push(GeneratedCodeEvidence {
                    character,
                    code: f[2].parse().map_err(code_error)?,
                    reading: f[3],
                    provenance,
                });
            }
            "frequency" => {
                let (kind, spelling) = if f[3] == "input-weight" {
                    (FrequencyKind::InputWeight, None)
                } else {
                    reading(f[3])?;
                    (FrequencyKind::ReadingScore, Some(f[3]))
                };
                evidence.frequencies.push(FrequencyEvidence {
                    character,
                    value: decimal(f[2])?,
                    reading: spelling,
                    kind,
                    provenance,
                });
            }
            "variant" => {
                let kind = match f[3] {
                    "simplified" => VariantKind::Simplified,
                    "traditional" => VariantKind::Traditional,
                    "compatibility" => VariantKind::Compatibility,
                    "other" => VariantKind::Other,
                    _ => return Err(invalid("unknown variant relation")),
                };
                let target = f[2].parse()?;
                if character == target {
                    return Err(invalid("self variant relation"));
                }
                evidence.variants.push(VariantEvidence {
                    character,
                    target,
                    kind,
                    provenance,
                });
            }
            _ => {
                return Err(invalid(format!(
                    "row {row}: unknown relation or invalid detail"
                )));
            }
        }
    }
    Ok(evidence)
}

pub fn serialize_evidence(e: &KnowledgeEvidence<'_>) -> Result<String, EvidenceError> {
    let mut lines = Vec::new();
    let mut add =
        |kind: &str, ch: CharacterId, value: String, detail: String, p: Provenance<'_>| {
            let confidence = match p.confidence {
                EvidenceConfidence::High => "high",
                EvidenceConfidence::Medium => "medium",
                EvidenceConfidence::Low => "low",
            };
            lines.push(format!(
                "{kind}\t{}\t{value}\t{detail}\t{}\t{}\t{confidence}",
                ch.0,
                p.source,
                p.status.as_str()
            ));
        };
    for r in &e.readings {
        add(
            "reading",
            r.character,
            r.reading.into(),
            if r.primary { "primary" } else { "alt" }.into(),
            r.provenance,
        );
    }
    for r in &e.sounds {
        add(
            "sound",
            r.character,
            r.code.to_string(),
            "-".into(),
            r.provenance,
        );
    }
    for r in &e.shapes {
        add(
            "shape",
            r.character,
            r.code.to_string(),
            "-".into(),
            r.provenance,
        );
    }
    for r in &e.full_codes {
        add(
            "full",
            r.character,
            r.code().to_string(),
            r.weight.to_string(),
            r.provenance,
        );
    }
    for r in &e.generated {
        add(
            "generated",
            r.character,
            r.code.to_string(),
            r.reading.into(),
            r.provenance,
        );
    }
    for r in &e.frequencies {
        if (r.kind == FrequencyKind::ReadingScore) != r.reading.is_some() {
            return Err(invalid("frequency kind and reading must agree"));
        }
        add(
            "frequency",
            r.character,
            r.value.to_string(),
            match r.kind {
                FrequencyKind::ReadingScore => r.reading.unwrap_or("").into(),
                FrequencyKind::InputWeight => "input-weight".into(),
            },
            r.provenance,
        );
    }
    for r in &e.variants {
        add(
            "variant",
            r.character,
            r.target.0.to_string(),
            match r.kind {
                VariantKind::Simplified => "simplified",
                VariantKind::Traditional => "traditional",
                VariantKind::Compatibility => "compatibility",
                VariantKind::Other => "other",
            }
            .into(),
            r.provenance,
        );
    }
    lines.sort();
    let mut text = HEADER.to_string();
    for line in lines {
        text.push_str(&line);
        text.push('\n');
    }
    parse_evidence(&text)?;
    Ok(text)
}
