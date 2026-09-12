use std::error::Error;
use xhup_analyzer::rules::{CANONICAL_RULE_FIXTURES, audit_rules};
use xhup_core::knowledge::{CharacterId, canonical_sources, parse_sources};
use xhup_core::rules::parse_fixtures;

fn main() -> Result<(), Box<dyn Error>> {
    let mut target = None;
    let mut json = false;
    let mut check = false;
    let mut timings = false;
    let mut fixture_path = None;
    let mut source_path = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--summary" => {}
            "--char" | "--word" => {
                if target.is_some() {
                    return Err("only one target is allowed".into());
                }
                let value = args.next().ok_or("missing target")?;
                if arg == "--char" {
                    value.parse::<CharacterId>()?;
                }
                if value.is_empty() || value.chars().any(|c| c.is_control() || c.is_whitespace()) {
                    return Err(
                        "target must be nonempty text without whitespace/control characters".into(),
                    );
                }
                target = Some(value);
            }
            "--json" => json = true,
            "--check" => check = true,
            "--timings" => timings = true,
            "--fixtures" => fixture_path = Some(args.next().ok_or("missing fixture path")?),
            "--sources" => source_path = Some(args.next().ok_or("missing source path")?),
            "--help" => {
                println!(
                    "xhup-rule-audit [--summary | --char 字 | --word 词] [--json] [--check] [--fixtures TSV] [--sources TSV] [--timings]"
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    let fixture_text = fixture_path.map(std::fs::read_to_string).transpose()?;
    let source_text = source_path.map(std::fs::read_to_string).transpose()?;
    let sources = source_text.as_deref().map(parse_sources).transpose()?;
    let sources = sources.as_deref().unwrap_or(canonical_sources());
    let started = std::time::Instant::now();
    let fixtures = parse_fixtures(
        fixture_text.as_deref().unwrap_or(CANONICAL_RULE_FIXTURES),
        sources,
    )?;
    let parse_ms = started.elapsed().as_secs_f64() * 1000.0;
    let report = audit_rules(&fixtures, sources, target.as_deref())?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("scope: {}", report.scope);
        for (name, count) in &report.global_counts {
            println!("{name}: {count}");
        }
        if let Some(text) = &report.text {
            println!(
                "text: {text}\nofficial expected codes: {}\nattested codes: {}\nproduction codes: {}\nFlow extension codes (preferred/fixture paths): {}",
                report.official_expected_codes.join(", "),
                report.attested_codes.join(", "),
                report.production_codes.join(", "),
                report.flow_extension_codes.join(", ")
            );
            for e in &report.explanations {
                println!(
                    "{}: {} [{}] {} -> {} ({})\n  rule-source: {}@{}; evidence: {}@{}\n  {}",
                    e.fixture_id,
                    e.rule,
                    e.compatibility_class,
                    e.components,
                    e.derived_code.as_deref().unwrap_or("unresolved"),
                    e.difference,
                    e.rule_source,
                    e.rule_source_revision,
                    e.evidence_source,
                    e.evidence_revision,
                    e.note
                );
            }
            if let Some(knowledge) = &report.knowledge {
                println!("knowledge:\n{}", knowledge.normalized_evidence);
            }
            println!("sources:\n{}", report.sources_tsv);
        }
    }
    if timings {
        eprintln!(
            "parser_ms={parse_ms:.3} total_ms={:.3}",
            started.elapsed().as_secs_f64() * 1000.0
        );
    }
    if check && report.global_counts["regressions"] != 0 {
        return Err("rule compatibility regression".into());
    }
    Ok(())
}
