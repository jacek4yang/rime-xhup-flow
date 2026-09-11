use std::collections::BTreeSet;
use std::error::Error;
use xhup_analyzer::knowledge::{audit_knowledge, explain_character};
use xhup_core::{InputHanzi, XhupHanzi, knowledge::*};

fn main() -> Result<(), Box<dyn Error>> {
    let mut ch = None;
    let mut json = false;
    let mut export = false;
    let mut timings = false;
    let mut check = false;
    let mut evidence_path = None;
    let mut sources_path = None;
    let mut high_frequency = 100_000;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--char" => {
                ch = Some(
                    args.next()
                        .ok_or("missing character")?
                        .parse::<CharacterId>()?
                        .0,
                )
            }
            "--json" => json = true,
            "--export-tsv" => export = true,
            "--timings" => timings = true,
            "--check" => check = true,
            "--evidence" => evidence_path = Some(args.next().ok_or("missing evidence path")?),
            "--sources" => sources_path = Some(args.next().ok_or("missing source path")?),
            "--high-frequency" => {
                high_frequency = decimal(&args.next().ok_or("missing threshold")?)?
            }
            "--help" => {
                println!(
                    "knowledge-audit [--char 字] [--json | --export-tsv] [--evidence TSV] [--sources TSV] [--high-frequency N] [--check] [--timings]"
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    if export && (json || ch.is_some()) {
        return Err("--export-tsv cannot be combined with --json or --char".into());
    }
    let evidence_text = evidence_path.map(std::fs::read_to_string).transpose()?;
    let source_text = sources_path.map(std::fs::read_to_string).transpose()?;
    let sources = source_text.as_deref().map(parse_sources).transpose()?;
    let sources = sources.as_deref().unwrap_or(canonical_sources());
    let started = std::time::Instant::now();
    let evidence = if let Some(text) = evidence_text.as_deref() {
        parse_evidence(text)?
    } else {
        xhup_generator::knowledge::canonical_knowledge()?
    };
    if timings {
        let adapter_ms = started.elapsed().as_secs_f64() * 1000.0;
        let normalized = serialize_evidence(&evidence)?;
        let parse_started = std::time::Instant::now();
        let _parsed = parse_evidence(&normalized)?;
        eprintln!(
            "evidence_bytes={} adapter_ms={adapter_ms:.3} parser_ms={:.3}",
            normalized.len(),
            parse_started.elapsed().as_secs_f64() * 1000.0
        );
    }
    if export {
        print!("{}", serialize_evidence(&evidence)?);
        return Ok(());
    }
    let core: BTreeSet<_> = XhupHanzi::all().iter().map(|c| c.as_char()).collect();
    let production: BTreeSet<_> = InputHanzi::all().iter().map(|c| c.as_char()).collect();
    if check {
        let report = audit_knowledge(&evidence, sources, &core, &production, high_frequency);
        if report.counts["duplicate_evidence"] > 0 || report.counts["invalid_provenance"] > 0 {
            return Err(
                "knowledge validation failed: duplicate evidence or invalid provenance".into(),
            );
        }
    }
    let audit_started = std::time::Instant::now();
    if let Some(ch) = ch {
        let report = explain_character(ch, &evidence, sources, &core, &production)?;
        if json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            println!(
                "character: {}\ncore-standard: {}\ninput-member: {}",
                ch, report.core_standard, report.input_member
            );
            println!(
                "evidence (kind / character / value / detail / source / status / confidence):\n{}",
                report.normalized_evidence
            );
            println!(
                "resolved full codes: {}",
                report.resolved_full_codes.join(", ")
            );
            println!(
                "preferred sources: {:?}\nsources:\n{}",
                report.preferred_sources, report.sources_tsv
            );
            for finding in report.findings {
                println!("{}: {}", finding.category, finding.details);
            }
        }
    } else {
        let report = audit_knowledge(&evidence, sources, &core, &production, high_frequency);
        if json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            for (name, count) in report.counts {
                println!("{name}: {count}");
            }
        }
    }
    if timings {
        eprintln!(
            "audit_ms={:.3}",
            audit_started.elapsed().as_secs_f64() * 1000.0
        );
    }
    Ok(())
}
