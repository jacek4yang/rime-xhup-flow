use std::collections::BTreeSet;

use serde::Serialize;
use xhup_core::knowledge::{EvidenceSource, canonical_sources, serialize_sources};
use xhup_core::{InputHanzi, XhupHanzi};

use super::*;

pub const CANONICAL_RULE_FIXTURES: &str =
    include_str!("../../../../data/xhup/rules/fixtures-v1.tsv");

#[derive(Clone, Debug, Serialize)]
pub struct DerivationExplanation {
    pub fixture_id: String,
    pub text: String,
    pub rule: String,
    pub rule_version: String,
    pub rule_source: String,
    pub rule_source_revision: String,
    pub derivation: String,
    pub components: String,
    pub derived_code: Option<String>,
    pub expected_code: Option<String>,
    pub compatibility_class: String,
    pub evidence_source: String,
    pub evidence_revision: String,
    pub policy_source: Option<String>,
    pub expectation: String,
    pub difference: String,
    pub static_exact: bool,
    pub extended_exact: bool,
    pub open_composition: bool,
    pub note: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuleAuditReport {
    pub schema: &'static str,
    /// Counts classify the independent fixture set, not the entire unknown
    /// official dictionary. A targeted query does not narrow the global gate.
    pub scope: &'static str,
    pub text: Option<String>,
    pub counts: BTreeMap<&'static str, usize>,
    pub global_counts: BTreeMap<&'static str, usize>,
    pub official_expected_codes: Vec<String>,
    pub attested_codes: Vec<String>,
    pub production_codes: Vec<String>,
    pub flow_extension_codes: Vec<String>,
    pub explanations: Vec<DerivationExplanation>,
    pub knowledge: Option<crate::knowledge::CharacterExplanation>,
    pub sources_tsv: String,
}

fn observe(fixture: &RuleFixture<'_>) -> ProductionObservation {
    if matches!(
        fixture.rule.definition().domain,
        RuleDomain::Sound | RuleDomain::Shape
    ) {
        return ProductionObservation::default();
    }
    let Some(code) = &fixture.expected else {
        return ProductionObservation::default();
    };
    let paths = xhup_generator::classify_reachability(fixture.text, &code.to_string());
    ProductionObservation {
        static_exact: paths.static_reachable,
        extended_exact: paths.extended_lexicon_reachable,
        open_composition: paths.open_composition_reachable,
    }
}

/// Deterministic explanation view for future Trainer consumers. Expected codes
/// come only from the externally sourced fixture, never from production output.
pub fn explain_rule_fixture(
    fixture: &RuleFixture<'_>,
    sources: &[EvidenceSource<'_>],
    observed: ProductionObservation,
) -> Result<DerivationExplanation, String> {
    let definition = fixture.rule.definition();
    let rule_source = sources
        .iter()
        .find(|s| s.id == definition.source)
        .ok_or("orphan rule source")?;
    let source = sources
        .iter()
        .find(|s| s.id == fixture.source)
        .ok_or("orphan fixture source")?;
    Ok(DerivationExplanation {
        fixture_id: fixture.id.into(),
        text: fixture.text.into(),
        rule: fixture.rule.as_str().into(),
        rule_version: definition.version.into(),
        rule_source: rule_source.id.into(),
        rule_source_revision: rule_source.revision.into(),
        derivation: definition.derivation.into(),
        components: fixture.components.canonical(),
        derived_code: derive_fixture(fixture)?.map(|c| c.to_string()),
        expected_code: fixture.expected.as_ref().map(ToString::to_string),
        compatibility_class: fixture.class.as_str().into(),
        evidence_source: source.id.into(),
        evidence_revision: source.revision.into(),
        policy_source: fixture.policy.map(str::to_string),
        expectation: fixture.expectation.as_str().into(),
        difference: compare_fixture(fixture, observed).as_str().into(),
        static_exact: observed.static_exact,
        extended_exact: observed.extended_exact,
        open_composition: observed.open_composition,
        note: fixture.note.into(),
    })
}

pub fn audit_rules(
    fixtures: &[RuleFixture<'_>],
    sources: &[EvidenceSource<'_>],
    target: Option<&str>,
) -> Result<RuleAuditReport, String> {
    // Revalidate public typed input, including deterministic ordering and sources.
    serialize_fixtures(fixtures, sources)?;
    let mut ordered: Vec<_> = fixtures.iter().collect();
    ordered.sort_by_key(|f| f.id);
    let mut all_differences = Vec::new();
    let mut differences = Vec::new();
    let mut explanations = Vec::new();
    let mut official_expected = BTreeSet::new();
    let mut flow_codes = BTreeSet::new();
    let mut missing_official = 0;
    for fixture in ordered {
        let observed = observe(fixture);
        let difference = compare_fixture(fixture, observed);
        all_differences.push(difference);
        if matches!(
            fixture.class,
            CompatibilityClass::ExactOfficial
                | CompatibilityClass::OfficialAlias
                | CompatibilityClass::OfficialSpecial
                | CompatibilityClass::IntentionalDeviation
        ) && fixture.rule.definition().layer == RuleLayer::Official
            && !matches!(
                fixture.rule.definition().domain,
                RuleDomain::Sound | RuleDomain::Shape
            )
            && !observed.reachable()
        {
            missing_official += 1;
        }
        if target.is_some_and(|text| text != fixture.text) {
            continue;
        }
        differences.push(difference);
        if let Some(code) = &fixture.expected {
            if matches!(
                fixture.class,
                CompatibilityClass::ExactOfficial
                    | CompatibilityClass::OfficialAlias
                    | CompatibilityClass::OfficialSpecial
                    | CompatibilityClass::IntentionalDeviation
            ) && !matches!(
                fixture.rule.definition().domain,
                RuleDomain::Shape | RuleDomain::Sound
            ) {
                official_expected.insert(code.to_string());
            }
            if fixture.class == CompatibilityClass::FlowExtension {
                flow_codes.insert(code.to_string());
            }
        }
        explanations.push(explain_rule_fixture(fixture, sources, observed)?);
    }
    let mut counts = summarize_differences(&differences);
    let mut global_counts = summarize_differences(&all_differences);
    global_counts.insert("distinct_sources", sources.len());
    global_counts.insert("missing_expected_official_relations", missing_official);
    counts.insert(
        "unknown_target",
        usize::from(target.is_some() && explanations.is_empty()),
    );
    let mut production_codes = BTreeSet::new();
    let mut attested_codes = BTreeSet::new();
    let mut knowledge = None;
    if let Some(text) = target {
        for entry in xhup_generator::canonical_input_char_code_entries() {
            if entry.hanzi().as_char().to_string() == text {
                production_codes.insert(entry.code().to_string());
            }
        }
        for entry in xhup_generator::canonical_level1_shortcuts() {
            if entry.hanzi().as_char().to_string() == text {
                production_codes.insert(entry.key().as_char().to_string());
            }
        }
        for entry in xhup_generator::canonical_word_code_entries() {
            if entry.word() == text {
                production_codes.insert(entry.code().to_string());
            }
        }
        for entry in xhup_generator::canonical_extended_word_code_entries() {
            if entry.word() == text {
                production_codes.insert(entry.code().to_string());
            }
        }
        for entry in xhup_generator::canonical_primary_shortcut_entries() {
            if entry.word() == text {
                production_codes.insert(entry.shortcut_code().to_string());
            }
        }
        for entry in xhup_generator::canonical_fixed_first_shortcut_entries() {
            if entry.word() == text {
                production_codes.insert(entry.shortcut_code().to_string());
            }
        }
        if text.chars().count() >= 2 {
            if let Some(code) = xhup_generator::preferred_open_composition_code(text) {
                flow_codes.insert(code);
            }
        } else if let Some(character) = text.chars().next() {
            let evidence =
                xhup_generator::knowledge::canonical_knowledge().map_err(|e| e.to_string())?;
            for relation in &evidence.full_codes {
                if relation.character.0 == character {
                    attested_codes.insert(relation.code().to_string());
                }
            }
            let core = XhupHanzi::all().iter().map(|c| c.as_char()).collect();
            let input = InputHanzi::all().iter().map(|c| c.as_char()).collect();
            knowledge = Some(
                crate::knowledge::explain_character(
                    character,
                    &evidence,
                    canonical_sources(),
                    &core,
                    &input,
                )
                .map_err(|e| e.to_string())?,
            );
        }
    }
    Ok(RuleAuditReport {
        schema: "xhup-rule-audit/v1",
        scope: "independent fixtures; not complete official dictionary coverage",
        text: target.map(str::to_string),
        counts,
        global_counts,
        official_expected_codes: official_expected.into_iter().collect(),
        attested_codes: attested_codes.into_iter().collect(),
        production_codes: production_codes.into_iter().collect(),
        flow_extension_codes: flow_codes.into_iter().collect(),
        explanations,
        knowledge,
        sources_tsv: serialize_sources(sources).map_err(|e| e.to_string())?,
    })
}
