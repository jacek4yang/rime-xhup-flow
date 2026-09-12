//! XHUP-specific rules. This layer explains derivations; it does not assign
//! dictionary entries, alter production mappings, or infer linguistic readings.

use crate::{DoublePinyinCode, DoublePinyinLayout, FullCode, Key, KeySequence, ShapeCode};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum CompatibilityClass {
    ExactOfficial,
    OfficialAlias,
    OfficialSpecial,
    HistoricalCompatible,
    RuleCompatible,
    FlowExtension,
    IntentionalDeviation,
    Conflict,
    Unknown,
}

impl CompatibilityClass {
    pub const ALL: [Self; 9] = [
        Self::ExactOfficial,
        Self::OfficialAlias,
        Self::OfficialSpecial,
        Self::HistoricalCompatible,
        Self::RuleCompatible,
        Self::FlowExtension,
        Self::IntentionalDeviation,
        Self::Conflict,
        Self::Unknown,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExactOfficial => "exact-official",
            Self::OfficialAlias => "official-alias",
            Self::OfficialSpecial => "official-special",
            Self::HistoricalCompatible => "historical-compatible",
            Self::RuleCompatible => "rule-compatible",
            Self::FlowExtension => "flow-extension",
            Self::IntentionalDeviation => "intentional-deviation",
            Self::Conflict => "conflict",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuleLayer {
    Official,
    Historical,
    CompilerPolicy,
    FlowExtension,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuleDomain {
    Sound,
    Shape,
    Character,
    Phrase,
    Shortcut,
    SpecialCase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum RuleId {
    Sound,
    Shape,
    Character,
    PhraseTwo,
    PhraseThree,
    PhraseFourPlus,
    CharacterShortcut,
    PhraseShortcut,
    AttestedSpecial,
    HistoricalCharacter,
    SequentialComposition,
    MonotoneShortcut,
    LegacyShortcut,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuleDefinition {
    pub id: RuleId,
    pub version: &'static str,
    /// An identifier in the Knowledge Base EvidenceSource registry.
    pub source: &'static str,
    pub layer: RuleLayer,
    pub domain: RuleDomain,
    pub derivation: &'static str,
    pub special_case: bool,
}

impl RuleId {
    pub const ALL: [Self; 14] = [
        Self::Sound,
        Self::Shape,
        Self::Character,
        Self::PhraseTwo,
        Self::PhraseThree,
        Self::PhraseFourPlus,
        Self::CharacterShortcut,
        Self::PhraseShortcut,
        Self::AttestedSpecial,
        Self::HistoricalCharacter,
        Self::SequentialComposition,
        Self::MonotoneShortcut,
        Self::LegacyShortcut,
        Self::Unresolved,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sound => "xhup-sound",
            Self::Shape => "xhup-shape",
            Self::Character => "xhup-character",
            Self::PhraseTwo => "xhup-phrase-two",
            Self::PhraseThree => "xhup-phrase-three",
            Self::PhraseFourPlus => "xhup-phrase-four-plus",
            Self::CharacterShortcut => "xhup-character-shortcut",
            Self::PhraseShortcut => "xhup-phrase-shortcut",
            Self::AttestedSpecial => "xhup-attested-special",
            Self::HistoricalCharacter => "historical-character",
            Self::SequentialComposition => "flow-sequential-composition",
            Self::MonotoneShortcut => "flow-monotone-shortcut",
            Self::LegacyShortcut => "flow-legacy-shortcut",
            Self::Unresolved => "xhup-unresolved",
        }
    }

    pub fn definition(self) -> RuleDefinition {
        use RuleDomain as D;
        use RuleLayer as L;
        let (source, layer, domain, derivation, special_case) = match self {
            Self::Sound => (
                "flypy-help-up",
                L::Official,
                D::Sound,
                "initial key + final key; zero initials follow the spelling-length rule",
                false,
            ),
            Self::Shape => (
                "flypy-help-gz",
                L::Official,
                D::Shape,
                "first and last supplied eligible roots; prefer larger roots; strokes when necessary",
                false,
            ),
            Self::Character => (
                "flypy-help-xh",
                L::Official,
                D::Character,
                "two sound keys + two shape keys",
                false,
            ),
            Self::PhraseTwo => (
                "flypy-help-xh",
                L::Official,
                D::Phrase,
                "first two keys of each of two characters",
                false,
            ),
            Self::PhraseThree => (
                "flypy-help-xh",
                L::Official,
                D::Phrase,
                "first key of first two characters + two keys of last character",
                false,
            ),
            Self::PhraseFourPlus => (
                "flypy-help-xh",
                L::Official,
                D::Phrase,
                "first key of first three characters + first key of last character",
                false,
            ),
            Self::CharacterShortcut => (
                "flypy-help-xh",
                L::Official,
                D::Shortcut,
                "attested proper prefix of character full code; assignment is not inferred",
                false,
            ),
            Self::PhraseShortcut => (
                "flypy-help-xh",
                L::Official,
                D::Shortcut,
                "attested first-key abbreviation shorter than four keys; assignment is not inferred",
                false,
            ),
            Self::AttestedSpecial => (
                "flypy-official-ix",
                L::Official,
                D::SpecialCase,
                "use attested sound/shape relation; never manufacture a reading",
                true,
            ),
            Self::HistoricalCharacter => (
                "rime-fast-xhup",
                L::Historical,
                D::Character,
                "preserve pinned implementation's sound/shape relation",
                true,
            ),
            Self::SequentialComposition => (
                "flow-v1-policy",
                L::FlowExtension,
                D::Phrase,
                "concatenate every character's two-key input sound; lexicon-independent",
                false,
            ),
            Self::MonotoneShortcut => (
                "flow-v2-compiler-policy",
                L::CompilerPolicy,
                D::Shortcut,
                "F* I* with at least one I; assignment/ranking remains compiler policy",
                false,
            ),
            Self::LegacyShortcut => (
                "flow-v2-compiler-policy",
                L::CompilerPolicy,
                D::Shortcut,
                "legacy F/I projection; preserved assignment is not proof of official attestation",
                true,
            ),
            Self::Unresolved => (
                "flow-v1-policy",
                L::CompilerPolicy,
                D::SpecialCase,
                "insufficient independent evidence; no derived code",
                true,
            ),
        };
        RuleDefinition {
            id: self,
            version: "1",
            source,
            layer,
            domain,
            derivation,
            special_case,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ShapePrinciple {
    FirstLast,
    PreferLargerRoot,
    NoIntersectedRoot,
    NoInterruptedRoot,
    WalkingRadicalFirst,
    TraditionalRootEquivalent,
    RepeatSingleStroke,
}

impl ShapePrinciple {
    pub const ALL: [Self; 7] = [
        Self::FirstLast,
        Self::PreferLargerRoot,
        Self::NoIntersectedRoot,
        Self::NoInterruptedRoot,
        Self::WalkingRadicalFirst,
        Self::TraditionalRootEquivalent,
        Self::RepeatSingleStroke,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FirstLast => "first-last",
            Self::PreferLargerRoot => "prefer-larger-root",
            Self::NoIntersectedRoot => "no-intersected-root",
            Self::NoInterruptedRoot => "no-interrupted-root",
            Self::WalkingRadicalFirst => "walking-radical-first",
            Self::TraditionalRootEquivalent => "traditional-root-equivalent",
            Self::RepeatSingleStroke => "repeat-single-stroke",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stroke {
    Horizontal,
    Vertical,
    LeftFalling,
    Dot,
    Turning,
    RightFalling,
}

impl Stroke {
    pub fn key(self) -> Key {
        Key::from_char(match self {
            Self::Horizontal => 'a',
            Self::Vertical => 'l',
            Self::LeftFalling => 'p',
            Self::Dot => 'd',
            Self::Turning => 'v',
            Self::RightFalling => 'n',
        })
        .expect("literal XHUP key")
    }
}

/// A decomposition must come from evidence. No Unicode glyph inspection occurs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShapeDecomposition<'a> {
    pub first_root: &'a str,
    pub last_root: &'a str,
    pub code: ShapeCode,
    pub principles: Vec<ShapePrinciple>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OfficialMarker {
    YieldFull,
    OutsideCore,
    RareCharacterOrSound,
}

impl OfficialMarker {
    pub fn from_marker(marker: char) -> Option<Self> {
        match marker {
            '-' => Some(Self::YieldFull),
            '+' => Some(Self::OutsideCore),
            '*' => Some(Self::RareCharacterOrSound),
            _ => None,
        }
    }
}

/// Keyboard derivation, not a linguistic-reading assertion. Unknown input stays
/// unresolved rather than being coerced into a convenient syllable.
pub fn derive_sound(initial: Option<&str>, final_: &str) -> Option<DoublePinyinCode> {
    let layout = DoublePinyinLayout::canonical();
    match initial {
        Some(initial) => layout.compose(initial, final_),
        None => layout.zero_initial_code(final_),
    }
}

pub fn derive_character(sound: DoublePinyinCode, shape: ShapeCode) -> FullCode {
    FullCode::from_parts(sound, shape)
}

/// Official fixed-four-key phrase construction. Input sound codes are already
/// evidenced; this does not choose a reading or assert dictionary membership.
pub fn derive_phrase(sounds: &[DoublePinyinCode]) -> Option<(RuleId, FullCode)> {
    let (rule, keys) = match sounds {
        [a, b] => (
            RuleId::PhraseTwo,
            [
                a.as_slice()[0],
                a.as_slice()[1],
                b.as_slice()[0],
                b.as_slice()[1],
            ],
        ),
        [a, b, c] => (
            RuleId::PhraseThree,
            [
                a.as_slice()[0],
                b.as_slice()[0],
                c.as_slice()[0],
                c.as_slice()[1],
            ],
        ),
        [a, b, c, rest @ ..] if !rest.is_empty() => (
            RuleId::PhraseFourPlus,
            [
                a.as_slice()[0],
                b.as_slice()[0],
                c.as_slice()[0],
                rest.last()?.as_slice()[0],
            ],
        ),
        _ => return None,
    };
    Some((rule, FullCode::new(keys)))
}

/// Flow extension: no dictionary lookup and no fixed phrase-length ceiling.
pub fn derive_composition(sounds: &[DoublePinyinCode]) -> Option<KeySequence> {
    if sounds.len() < 2 {
        return None;
    }
    let keys: Vec<_> = sounds
        .iter()
        .flat_map(|s| s.as_slice().iter().copied())
        .collect();
    KeySequence::from_keys(&keys).ok()
}

/// This checks structural compatibility only, never shortcut assignment.
pub fn character_shortcut_matches(full: FullCode, shortcut: &KeySequence) -> bool {
    shortcut.len() < 4 && full.as_slice().starts_with(shortcut.as_slice())
}

pub fn phrase_shortcut_matches(sounds: &[DoublePinyinCode], shortcut: &KeySequence) -> bool {
    (2..4).contains(&sounds.len())
        && shortcut.len() == sounds.len()
        && sounds
            .iter()
            .zip(shortcut.iter())
            .all(|(sound, key)| sound.as_slice()[0] == *key)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sounds(codes: &[&str]) -> Vec<DoublePinyinCode> {
        codes.iter().map(|s| s.parse().unwrap()).collect()
    }
    #[test]
    fn official_examples_are_four_keys_not_sequential_flow_codes() {
        for (components, expected) in [
            (vec!["ul", "pb"], "ulpb"),
            (vec!["uu", "ru", "fa"], "urfa"),
            (vec!["ta", "xl", "yu", "gu", "vi"], "txyv"),
        ] {
            assert_eq!(
                derive_phrase(&sounds(&components)).unwrap().1.to_string(),
                expected
            );
        }
        assert_eq!(
            derive_phrase(&sounds(&["ti", "ui", "ci"]))
                .unwrap()
                .1
                .to_string(),
            "tuci"
        );
        assert_eq!(
            derive_composition(&sounds(&["ti", "ui", "ci"]))
                .unwrap()
                .to_string(),
            "tiuici"
        );
        assert!(derive_phrase(&[]).is_none());
        assert!(derive_phrase(&sounds(&["aa"])).is_none());
    }
    #[test]
    fn sound_derivation_does_not_invent_interjection_readings() {
        assert_eq!(derive_sound(Some("sh"), "uang").unwrap().to_string(), "ul");
        for (syllable, expected) in [("a", "aa"), ("ai", "ai"), ("ang", "ah")] {
            assert_eq!(derive_sound(None, syllable).unwrap().to_string(), expected);
        }
        assert!(derive_sound(None, "n").is_none());
        assert!(derive_sound(None, "ng").is_none());
        assert!(derive_sound(Some("SH"), "uang").is_none());
    }
    #[test]
    fn shape_and_special_case_semantics_are_explicit() {
        let stroke = Stroke::Turning.key();
        let shape = ShapeCode::new([stroke, stroke]);
        assert_eq!(
            derive_character("yi".parse().unwrap(), shape).to_string(),
            "yivv"
        );
        assert_eq!(
            OfficialMarker::from_marker('-'),
            Some(OfficialMarker::YieldFull)
        );
        assert!(OfficialMarker::from_marker('x').is_none());
        assert_eq!(
            RuleId::SequentialComposition.definition().layer,
            RuleLayer::FlowExtension
        );
        assert_eq!(
            RuleId::LegacyShortcut.definition().layer,
            RuleLayer::CompilerPolicy
        );
    }
    #[test]
    fn shortcut_structure_is_not_assignment_evidence() {
        assert!(character_shortcut_matches(
            "xnld".parse().unwrap(),
            &"x".parse().unwrap()
        ));
        assert!(!character_shortcut_matches(
            "xnld".parse().unwrap(),
            &"xnld".parse().unwrap()
        ));
        assert!(phrase_shortcut_matches(
            &sounds(&["yt", "ld", "yt"]),
            &"yly".parse().unwrap()
        ));
        assert!(!phrase_shortcut_matches(
            &sounds(&["ti", "ui", "ci"]),
            &"tiuici".parse().unwrap()
        ));
    }
}

mod fixtures;
pub use fixtures::*;
