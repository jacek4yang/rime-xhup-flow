use super::*;
use crate::knowledge::{EvidenceSource, SourceKind};
use crate::{DoublePinyinCode, FullCode, KeySequence, ShapeCode};
use std::collections::BTreeSet;

pub const FIXTURE_SCHEMA: &str = "# xhup-rule-fixtures/v1\n";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Expectation {
    Protected,
    Observe,
    KnownMissing,
    Unresolved,
}
impl Expectation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Protected => "protected",
            Self::Observe => "observe",
            Self::KnownMissing => "known-missing",
            Self::Unresolved => "unresolved",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Components<'a> {
    Sound {
        initial: Option<&'a str>,
        final_: &'a str,
    },
    Shape(ShapeDecomposition<'a>),
    Character {
        sound: DoublePinyinCode,
        shape: ShapeCode,
    },
    Phrase(Vec<DoublePinyinCode>),
    CharacterShortcut(FullCode),
    Projection {
        sounds: Vec<DoublePinyinCode>,
        modes: &'a str,
    },
    Unresolved,
}

impl Components<'_> {
    pub fn canonical(&self) -> String {
        match self {
            Self::Sound { initial, final_ } => format!("{},{}", initial.unwrap_or("-"), final_),
            Self::Shape(shape) => {
                let mut principles = shape.principles.clone();
                principles.sort();
                format!(
                    "{}:{},{}:{};{}",
                    shape.first_root,
                    shape.code.as_slice()[0],
                    shape.last_root,
                    shape.code.as_slice()[1],
                    principles
                        .iter()
                        .map(|p| p.as_str())
                        .collect::<Vec<_>>()
                        .join("+")
                )
            }
            Self::Character { sound, shape } => format!("{sound},{shape}"),
            Self::Phrase(sounds) => sound_list(sounds),
            Self::CharacterShortcut(code) => code.to_string(),
            Self::Projection { sounds, modes } => format!("{};{modes}", sound_list(sounds)),
            Self::Unresolved => "-".into(),
        }
    }
}

fn sound_list(sounds: &[DoublePinyinCode]) -> String {
    sounds
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleFixture<'a> {
    pub id: &'a str,
    pub text: &'a str,
    pub rule: RuleId,
    pub components: Components<'a>,
    pub expected: Option<KeySequence>,
    pub source: &'a str,
    pub class: CompatibilityClass,
    pub expectation: Expectation,
    /// Required for a known missing relation; identifies an explicit Flow policy.
    pub policy: Option<&'a str>,
    pub note: &'a str,
}

fn parse_sounds(input: &str) -> Result<Vec<DoublePinyinCode>, String> {
    input
        .split(',')
        .map(|code| code.parse().map_err(|e| format!("sound component: {e}")))
        .collect()
}

fn parse_components(rule: RuleId, raw: &str) -> Result<Components<'_>, String> {
    let pair = || {
        raw.split_once(',')
            .ok_or_else(|| "expected two components".to_string())
    };
    Ok(match rule {
        RuleId::Sound => {
            let (initial, final_) = pair()?;
            if final_.is_empty()
                || !final_.bytes().all(|c| c.is_ascii_lowercase())
                || (initial != "-"
                    && (initial.is_empty() || !initial.bytes().all(|c| c.is_ascii_lowercase())))
            {
                return Err("invalid sound spelling".into());
            }
            Components::Sound {
                initial: (initial != "-").then_some(initial),
                final_,
            }
        }
        RuleId::Shape => {
            let (roots, principle_text) = raw.split_once(';').ok_or("missing shape principles")?;
            let principles: Vec<_> = principle_text
                .split('+')
                .map(|name| {
                    ShapePrinciple::ALL
                        .into_iter()
                        .find(|p| p.as_str() == name)
                        .ok_or("unknown shape principle")
                })
                .collect::<Result<_, _>>()?;
            if principles.is_empty() || principles.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err("shape principles must be sorted and unique".into());
            }
            let (first, last) = roots.split_once(',').ok_or("expected two roots")?;
            let (first_root, first_key) = first.split_once(':').ok_or("missing first root key")?;
            let (last_root, last_key) = last.split_once(':').ok_or("missing last root key")?;
            if first_root.is_empty()
                || last_root.is_empty()
                || first_key.len() != 1
                || last_key.len() != 1
            {
                return Err("invalid root component".into());
            }
            let code = format!("{first_key}{last_key}")
                .parse()
                .map_err(|e| format!("shape: {e}"))?;
            Components::Shape(ShapeDecomposition {
                first_root,
                last_root,
                code,
                principles,
            })
        }
        RuleId::Character | RuleId::AttestedSpecial | RuleId::HistoricalCharacter => {
            let (sound, shape) = pair()?;
            Components::Character {
                sound: sound.parse().map_err(|e| format!("sound: {e}"))?,
                shape: shape.parse().map_err(|e| format!("shape: {e}"))?,
            }
        }
        RuleId::CharacterShortcut => {
            Components::CharacterShortcut(raw.parse().map_err(|e| format!("full code: {e}"))?)
        }
        RuleId::PhraseTwo
        | RuleId::PhraseThree
        | RuleId::PhraseFourPlus
        | RuleId::PhraseShortcut
        | RuleId::SequentialComposition => Components::Phrase(parse_sounds(raw)?),
        RuleId::MonotoneShortcut | RuleId::LegacyShortcut => {
            let (codes, modes) = raw.split_once(';').ok_or("missing projection modes")?;
            let sounds = parse_sounds(codes)?;
            if sounds.len() != modes.len()
                || !modes.bytes().all(|c| matches!(c, b'F' | b'I'))
                || !modes.contains('I')
                || (rule == RuleId::MonotoneShortcut && modes.contains("IF"))
            {
                return Err("invalid compiler projection".into());
            }
            Components::Projection { sounds, modes }
        }
        RuleId::Unresolved if raw == "-" => Components::Unresolved,
        RuleId::Unresolved => {
            return Err("unresolved rule cannot pretend to derive components".into());
        }
    })
}

/// A pure rule derivation. This never reads current production or the lexicon.
pub fn derive_fixture(f: &RuleFixture<'_>) -> Result<Option<KeySequence>, String> {
    let as_sequence = |value: String| value.parse().map(Some).map_err(|e| format!("code: {e}"));
    match &f.components {
        Components::Sound { initial, final_ } => derive_sound(*initial, final_)
            .map(|code| code.to_string())
            .map(as_sequence)
            .unwrap_or(Ok(None)),
        Components::Shape(shape) => as_sequence(shape.code.to_string()),
        Components::Character { sound, shape } => {
            as_sequence(derive_character(*sound, *shape).to_string())
        }
        Components::Phrase(sounds) => {
            if sounds.len() != f.text.chars().count() {
                return Err("text/component count mismatch".into());
            }
            match f.rule {
                RuleId::SequentialComposition => Ok(derive_composition(sounds)),
                RuleId::PhraseShortcut => {
                    let Some(code) = &f.expected else {
                        return Err("shortcut assignment absent".into());
                    };
                    if !phrase_shortcut_matches(sounds, code) {
                        return Err("invalid phrase shortcut".into());
                    }
                    Ok(Some(code.clone()))
                }
                _ => {
                    let Some((rule, code)) = derive_phrase(sounds) else {
                        return Err("invalid phrase length".into());
                    };
                    if rule != f.rule {
                        return Err("wrong phrase-length rule".into());
                    }
                    as_sequence(code.to_string())
                }
            }
        }
        Components::CharacterShortcut(full) => {
            let Some(code) = &f.expected else {
                return Err("shortcut assignment absent".into());
            };
            if !character_shortcut_matches(*full, code) {
                return Err("invalid character shortcut".into());
            }
            Ok(Some(code.clone()))
        }
        Components::Projection { sounds, modes } => {
            if sounds.len() != f.text.chars().count() || sounds.len() < 2 {
                return Err("projection text/component count mismatch".into());
            }
            let keys: Vec<_> = sounds
                .iter()
                .zip(modes.bytes())
                .flat_map(|(sound, mode)| {
                    sound.as_slice()[..if mode == b'F' { 2 } else { 1 }]
                        .iter()
                        .copied()
                })
                .collect();
            KeySequence::from_keys(&keys)
                .map(Some)
                .map_err(|e| e.to_string())
        }
        Components::Unresolved => Ok(None),
    }
}

pub fn parse_fixtures<'a>(
    text: &'a str,
    sources: &[EvidenceSource<'_>],
) -> Result<Vec<RuleFixture<'a>>, String> {
    if text.contains('\r') || !text.ends_with('\n') {
        return Err("fixtures require LF and final newline".into());
    }
    let body = text
        .strip_prefix(FIXTURE_SCHEMA)
        .ok_or("unsupported fixture schema")?;
    let mut fixtures: Vec<RuleFixture<'a>> = Vec::new();
    let mut semantic = BTreeSet::new();
    for (index, line) in body.lines().enumerate() {
        let fail = |message: &str| format!("fixture row {}: {message}", index + 2);
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 10
            || fields
                .iter()
                .any(|f| f.is_empty() || *f != f.trim() || f.contains(char::is_control))
        {
            return Err(fail("expected ten nonempty canonical fields"));
        }
        let id = fields[0];
        if !id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || fixtures.last().is_some_and(|previous| previous.id >= id)
        {
            return Err(fail("ids must be unique, ASCII and sorted"));
        }
        let rule = RuleId::ALL
            .into_iter()
            .find(|r| r.as_str() == fields[2])
            .ok_or_else(|| fail("unknown rule"))?;
        let class = CompatibilityClass::ALL
            .into_iter()
            .find(|c| c.as_str() == fields[6])
            .ok_or_else(|| fail("unknown class"))?;
        let expectation = match fields[7] {
            "protected" => Expectation::Protected,
            "observe" => Expectation::Observe,
            "known-missing" => Expectation::KnownMissing,
            "unresolved" => Expectation::Unresolved,
            _ => return Err(fail("unknown expectation")),
        };
        let source = sources
            .iter()
            .find(|s| s.id == fields[5])
            .ok_or_else(|| fail("orphan fixture provenance"))?;
        let definition = rule.definition();
        if !sources.iter().any(|s| s.id == definition.source) {
            return Err(fail("orphan rule provenance"));
        }
        if matches!(
            class,
            CompatibilityClass::ExactOfficial
                | CompatibilityClass::OfficialAlias
                | CompatibilityClass::OfficialSpecial
        ) && (source.kind != SourceKind::Oracle || definition.layer != RuleLayer::Official)
        {
            return Err(fail(
                "official classification requires official rule and oracle evidence",
            ));
        }
        if class == CompatibilityClass::FlowExtension
            && definition.layer != RuleLayer::FlowExtension
        {
            return Err(fail("Flow extension requires an extension rule"));
        }
        let policy = (fields[8] != "-").then_some(fields[8]);
        if let Some(policy) = policy
            && !sources
                .iter()
                .any(|s| s.id == policy && s.kind == SourceKind::Project)
        {
            return Err(fail("policy must reference a project source"));
        }
        if (expectation == Expectation::KnownMissing)
            != (class == CompatibilityClass::IntentionalDeviation)
            || (expectation == Expectation::KnownMissing && policy.is_none())
        {
            return Err(fail(
                "intentional missing relation requires explicit policy",
            ));
        }
        if matches!(definition.domain, RuleDomain::Character | RuleDomain::Shape)
            || matches!(rule, RuleId::CharacterShortcut | RuleId::AttestedSpecial)
        {
            let mut chars = fields[1].chars();
            if chars.next().is_none() || chars.next().is_some() {
                return Err(fail("character domain requires one scalar"));
            }
        }
        let expected = if fields[4] == "-" {
            None
        } else {
            Some(
                fields[4]
                    .parse()
                    .map_err(|e| fail(&format!("invalid expected code: {e}")))?,
            )
        };
        if (expectation == Expectation::Unresolved) != expected.is_none()
            || (expectation == Expectation::Unresolved && class != CompatibilityClass::Unknown)
        {
            return Err(fail(
                "unresolved expectation requires unknown classification and no code",
            ));
        }
        if !semantic.insert((fields[1], rule, fields[3], source.id)) {
            return Err(fail("duplicate semantic fixture"));
        }
        let fixture = RuleFixture {
            id,
            text: fields[1],
            rule,
            components: parse_components(rule, fields[3]).map_err(|e| fail(&e))?,
            expected,
            source: fields[5],
            class,
            expectation,
            policy,
            note: fields[9],
        };
        if fixture.components.canonical() != fields[3] {
            return Err(fail("noncanonical components"));
        }
        if derive_fixture(&fixture).map_err(|e| fail(&e))? != fixture.expected {
            return Err(fail(
                "independent expected code disagrees with rule derivation",
            ));
        }
        fixtures.push(fixture);
    }
    Ok(fixtures)
}

pub fn serialize_fixtures(
    fixtures: &[RuleFixture<'_>],
    sources: &[EvidenceSource<'_>],
) -> Result<String, String> {
    let mut sorted: Vec<_> = fixtures.iter().collect();
    sorted.sort_by_key(|f| f.id);
    let mut text = FIXTURE_SCHEMA.to_string();
    for f in sorted {
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            f.id,
            f.text,
            f.rule.as_str(),
            f.components.canonical(),
            f.expected
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "-".into()),
            f.source,
            f.class.as_str(),
            f.expectation.as_str(),
            f.policy.unwrap_or("-"),
            f.note
        ));
    }
    parse_fixtures(&text, sources)?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::canonical_sources;
    fn sources() -> Vec<EvidenceSource<'static>> {
        canonical_sources().to_vec()
    }
    const ROW: &str = "example\t输入法\txhup-phrase-three\tuu,ru,fa\turfa\tflypy-help-xh\texact-official\tprotected\t-\tIndependent example\n";
    #[test]
    fn fixture_round_trip_is_sorted_and_deterministic() {
        let sources = sources();
        let data = format!("{FIXTURE_SCHEMA}{ROW}");
        let fixtures = parse_fixtures(&data, &sources).unwrap();
        assert_eq!(serialize_fixtures(&fixtures, &sources).unwrap(), data);
        assert_eq!(
            derive_fixture(&fixtures[0]).unwrap().unwrap().to_string(),
            "urfa"
        );
        let extended = format!(
            "{data}{}",
            ROW.replace("example", "other")
                .replace("输入法", "甲乙丙")
                .replace("uu,ru,fa", "aa,bb,cc")
                .replace("urfa", "abcc")
        );
        let mut fixtures = parse_fixtures(&extended, &sources).unwrap();
        fixtures.reverse();
        assert_eq!(serialize_fixtures(&fixtures, &sources).unwrap(), extended);
    }
    #[test]
    fn parser_rejects_malformed_duplicate_and_orphan_fixtures() {
        let sources = sources();
        let data = format!("{FIXTURE_SCHEMA}{ROW}");
        for bad in [
            data.replace("/v1", "/v2"),
            data.replace('\n', "\r\n"),
            data.trim_end().to_string(),
            format!("{data}{ROW}"),
            format!("{data}{}", ROW.replace("example", "other")),
            data.replace("example", "EXAMPLE"),
            data.replace("urfa", "urFa"),
            data.replace("urfa", "uufa"),
            data.replace("flypy-help-xh", "orphan"),
            data.replace("protected", "known-missing"),
            data.replace("uu,ru,fa", "uu,ru"),
            data.replace("phrase-three", "phrase-two"),
            data.replace("exact-official", "flow-extension"),
            data.replace("flypy-help-xh", "rime-fast-xhup"),
            data.replace("输入法", "输入"),
            data.replace("\t-\t", "\t\t"),
        ] {
            assert!(parse_fixtures(&bad, &sources).is_err(), "{bad}");
        }
        let missing_rule_source: Vec<_> = sources
            .iter()
            .filter(|s| s.id != "flypy-help-xh")
            .cloned()
            .collect();
        assert!(
            parse_fixtures(
                &data.replace("flypy-help-xh", "flypy-help-gz"),
                &missing_rule_source
            )
            .is_err()
        );
    }
    #[test]
    fn known_deviations_require_explicit_policy_and_unknowns_cannot_guess() {
        let sources = sources();
        let data = format!(
            "{FIXTURE_SCHEMA}{}",
            ROW.replace("exact-official", "intentional-deviation")
                .replace("protected", "known-missing")
                .replace("\t-\t", "\tflow-v1-policy\t")
        );
        assert!(parse_fixtures(&data, &sources).is_ok());
        assert!(
            parse_fixtures(&data.replace("flow-v1-policy", "rime-fast-xhup"), &sources).is_err()
        );
        let unknown = format!(
            "{FIXTURE_SCHEMA}unknown\t嗯\txhup-unresolved\t-\t-\tcore-readings\tunknown\tunresolved\t-\tNo ordinary sound derivation from n\n"
        );
        assert!(parse_fixtures(&unknown, &sources).is_ok());
        assert!(parse_fixtures(&unknown.replace("\t-\t-\t", "\tn\tnn\t"), &sources).is_err());
    }
    #[test]
    fn flow_projection_is_separate_from_official_phrase_construction() {
        let sources = sources();
        let data = format!(
            "{FIXTURE_SCHEMA}flow\t提示词\tflow-sequential-composition\tti,ui,ci\ttiuici\tflow-v1-policy\tflow-extension\tprotected\t-\tOpen composition\n"
        );
        assert!(parse_fixtures(&data, &sources).is_ok());
        assert!(
            parse_fixtures(&data.replace("flow-extension", "exact-official"), &sources).is_err()
        );
        let legacy = format!(
            "{FIXTURE_SCHEMA}legacy\t如果\tflow-legacy-shortcut\tru,go;IF\trgo\tflow-v1-policy\thistorical-compatible\tprotected\t-\tFrozen v1 policy alias\n"
        );
        assert!(parse_fixtures(&legacy, &sources).is_ok());
        assert!(
            parse_fixtures(
                &legacy.replace("flow-legacy-shortcut", "flow-monotone-shortcut"),
                &sources
            )
            .is_err()
        );
    }
    #[test]
    fn external_fixtures_are_canonical() {
        let sources = canonical_sources();
        let data = include_str!("../../../../data/xhup/rules/fixtures-v1.tsv");
        let fixtures = parse_fixtures(data, sources).unwrap();
        assert_eq!(serialize_fixtures(&fixtures, sources).unwrap(), data);
        assert_eq!(
            fixtures
                .iter()
                .filter(|f| f.expectation == Expectation::Protected)
                .count(),
            31,
            "reviewed protection labels must not be silently relaxed"
        );
        let shape = fixtures.iter().find(|f| f.id == "shape-yi").unwrap();
        let Components::Shape(decomposition) = &shape.components else {
            panic!("typed shape")
        };
        assert!(
            decomposition
                .principles
                .contains(&ShapePrinciple::RepeatSingleStroke)
        );
        assert!(
            parse_fixtures(
                &data.replace(
                    "first-last+repeat-single-stroke",
                    "repeat-single-stroke+first-last"
                ),
                sources
            )
            .is_err()
        );
        assert!(
            parse_fixtures(
                &data.replace("first-last+repeat-single-stroke", "first-last+first-last"),
                sources
            )
            .is_err()
        );
    }

    #[test]
    fn conflicting_sources_coexist_without_last_source_wins() {
        let text = format!(
            "{FIXTURE_SCHEMA}a\t甲\txhup-character\taa,aa\taaaa\tflypy-help-xh\tconflict\tobserve\t-\tSynthetic conflicting oracle\nb\t甲\thistorical-character\tbb,bb\tbbbb\trime-fast-xhup\tconflict\tobserve\t-\tSynthetic conflicting implementation\n"
        );
        let mut fixtures = parse_fixtures(&text, canonical_sources()).unwrap();
        assert_eq!(fixtures.len(), 2);
        fixtures.reverse();
        assert_eq!(
            serialize_fixtures(&fixtures, canonical_sources()).unwrap(),
            text
        );
    }
}
