use std::collections::BTreeMap;
use xhup_core::rules::*;

mod report;
pub use report::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProductionObservation {
    pub static_exact: bool,
    pub extended_exact: bool,
    pub open_composition: bool,
}
impl ProductionObservation {
    pub fn reachable(self) -> bool {
        self.static_exact || self.extended_exact || self.open_composition
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Difference {
    ExactOfficialMatch,
    OfficialAliasMatch,
    OfficialSpecialMatch,
    HistoricalMatch,
    RuleCompatibleMatch,
    FlowExtension,
    IntentionalDeviation,
    DeviationResolved,
    RuleVerified,
    RuleOnly,
    MissingProtected,
    UnclassifiedBreaking,
    Conflict,
    Unknown,
}
impl Difference {
    pub const ALL: [Self; 14] = [
        Self::ExactOfficialMatch,
        Self::OfficialAliasMatch,
        Self::OfficialSpecialMatch,
        Self::HistoricalMatch,
        Self::RuleCompatibleMatch,
        Self::FlowExtension,
        Self::IntentionalDeviation,
        Self::DeviationResolved,
        Self::RuleVerified,
        Self::RuleOnly,
        Self::MissingProtected,
        Self::UnclassifiedBreaking,
        Self::Conflict,
        Self::Unknown,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExactOfficialMatch => "exact_official_matches",
            Self::OfficialAliasMatch => "accepted_official_aliases",
            Self::OfficialSpecialMatch => "official_special_matches",
            Self::HistoricalMatch => "legacy_compatible_relations",
            Self::RuleCompatibleMatch => "rule_compatible_matches",
            Self::FlowExtension => "flow_extensions",
            Self::IntentionalDeviation => "intentional_deviations",
            Self::DeviationResolved => "deviations_resolved",
            Self::RuleVerified => "independent_rule_verifications",
            Self::RuleOnly => "rule_only_without_attestation",
            Self::MissingProtected => "missing_protected_relations",
            Self::UnclassifiedBreaking => "unclassified_breaking_differences",
            Self::Conflict => "conflicts",
            Self::Unknown => "unknown_unresolved_cases",
        }
    }
    pub fn is_regression(self) -> bool {
        matches!(self, Self::MissingProtected | Self::UnclassifiedBreaking)
    }
}

/// Extra production paths are deliberately absent from the failure predicate.
/// Protected static ordering is checked separately by the frozen v1 snapshot.
pub fn compare_fixture(fixture: &RuleFixture<'_>, observed: ProductionObservation) -> Difference {
    if fixture.expectation == Expectation::Unresolved {
        return Difference::Unknown;
    }
    if matches!(
        fixture.rule.definition().domain,
        RuleDomain::Sound | RuleDomain::Shape
    ) {
        return Difference::RuleVerified;
    }
    if fixture.expectation == Expectation::KnownMissing {
        return if observed.reachable() {
            Difference::DeviationResolved
        } else {
            Difference::IntentionalDeviation
        };
    }
    if !observed.reachable() {
        return if fixture.expectation == Expectation::Protected {
            Difference::MissingProtected
        } else if fixture.class == CompatibilityClass::RuleCompatible {
            Difference::RuleOnly
        } else {
            Difference::UnclassifiedBreaking
        };
    }
    match fixture.class {
        CompatibilityClass::ExactOfficial => Difference::ExactOfficialMatch,
        CompatibilityClass::OfficialAlias => Difference::OfficialAliasMatch,
        CompatibilityClass::OfficialSpecial => Difference::OfficialSpecialMatch,
        CompatibilityClass::HistoricalCompatible => Difference::HistoricalMatch,
        CompatibilityClass::RuleCompatible => Difference::RuleCompatibleMatch,
        CompatibilityClass::FlowExtension => Difference::FlowExtension,
        CompatibilityClass::IntentionalDeviation => Difference::DeviationResolved,
        CompatibilityClass::Conflict => Difference::Conflict,
        CompatibilityClass::Unknown => Difference::Unknown,
    }
}

pub fn summarize_differences(differences: &[Difference]) -> BTreeMap<&'static str, usize> {
    let mut counts: BTreeMap<_, _> = Difference::ALL
        .into_iter()
        .map(|d| (d.as_str(), 0))
        .collect();
    for difference in differences {
        *counts.get_mut(difference.as_str()).expect("known category") += 1;
    }
    counts.insert("fixture_cases", differences.len());
    counts.insert(
        "regressions",
        differences.iter().filter(|d| d.is_regression()).count(),
    );
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> RuleFixture<'static> {
        RuleFixture {
            id: "example",
            text: "小",
            rule: RuleId::Character,
            components: Components::Character {
                sound: "xn".parse().unwrap(),
                shape: "ld".parse().unwrap(),
            },
            expected: Some("xnld".parse().unwrap()),
            source: "flypy-help-xh",
            class: CompatibilityClass::ExactOfficial,
            expectation: Expectation::Protected,
            policy: None,
            note: "synthetic differential fixture",
        }
    }
    #[test]
    fn missing_protected_code_is_regression_but_extra_flow_path_is_not() {
        let fixture = fixture();
        assert!(compare_fixture(&fixture, ProductionObservation::default()).is_regression());
        assert_eq!(
            compare_fixture(
                &fixture,
                ProductionObservation {
                    static_exact: true,
                    ..Default::default()
                }
            ),
            Difference::ExactOfficialMatch
        );
        assert_eq!(
            compare_fixture(
                &fixture,
                ProductionObservation {
                    static_exact: true,
                    open_composition: true,
                    ..Default::default()
                }
            ),
            Difference::ExactOfficialMatch
        );
    }
    #[test]
    fn known_deviation_is_not_silently_generalized_to_missing_official_codes() {
        let mut fixture = fixture();
        fixture.class = CompatibilityClass::IntentionalDeviation;
        fixture.expectation = Expectation::KnownMissing;
        fixture.policy = Some("flow-v1-policy");
        assert_eq!(
            compare_fixture(&fixture, ProductionObservation::default()),
            Difference::IntentionalDeviation
        );
        fixture.expectation = Expectation::Observe;
        fixture.class = CompatibilityClass::ExactOfficial;
        assert!(compare_fixture(&fixture, ProductionObservation::default()).is_regression());
    }
    #[test]
    fn rule_compatibility_does_not_assert_official_dictionary_membership() {
        let mut fixture = fixture();
        fixture.class = CompatibilityClass::RuleCompatible;
        fixture.expectation = Expectation::Observe;
        assert_eq!(
            compare_fixture(&fixture, ProductionObservation::default()),
            Difference::RuleOnly
        );
    }
    #[test]
    fn summaries_are_stable_and_include_zero_categories() {
        let differences = [
            Difference::ExactOfficialMatch,
            Difference::Unknown,
            Difference::FlowExtension,
        ];
        let mut reversed = differences;
        reversed.reverse();
        assert_eq!(
            summarize_differences(&differences),
            summarize_differences(&reversed)
        );
        assert_eq!(summarize_differences(&differences)["regressions"], 0);
        assert_eq!(
            summarize_differences(&differences)["missing_protected_relations"],
            0
        );
    }
}
