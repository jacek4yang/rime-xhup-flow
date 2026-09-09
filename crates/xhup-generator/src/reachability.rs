//! 输入可达性分类。
//!
//! 固定词库只是一条更高质量路径；逐字两键音码组合是独立的开放路径。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::{
    canonical_extended_word_code_entries, canonical_fixed_first_shortcut_entries,
    canonical_input_char_code_entries, canonical_level1_shortcuts,
    canonical_primary_shortcut_entries, canonical_word_code_entries,
};

/// 一个 `(text, code)` 可通过哪些生产路径到达。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Reachability {
    pub static_reachable: bool,
    pub extended_lexicon_reachable: bool,
    pub open_composition_reachable: bool,
    pub sentence_reachable: bool,
}

impl Reachability {
    /// 至少存在一条生产输入路径。
    pub fn is_reachable(self) -> bool {
        self.static_reachable
            || self.extended_lexicon_reachable
            || self.open_composition_reachable
            || self.sentence_reachable
    }
}

struct Index {
    static_exact: BTreeSet<(String, String)>,
    extended_exact: BTreeSet<(String, String)>,
    sound_primitives: BTreeSet<(char, String)>,
    preferred_sound: BTreeMap<char, String>,
}

fn index() -> &'static Index {
    static INDEX: OnceLock<Index> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut static_exact = BTreeSet::new();
        let mut sound_primitives = BTreeSet::new();
        let mut preferred: BTreeMap<char, (bool, u64, String)> = BTreeMap::new();
        for entry in canonical_level1_shortcuts() {
            static_exact.insert((
                entry.hanzi().as_char().to_string(),
                entry.key().as_char().to_string(),
            ));
        }
        for entry in canonical_input_char_code_entries() {
            static_exact.insert((
                entry.hanzi().as_char().to_string(),
                entry.code().to_string(),
            ));
            if entry.code().len() == 2 {
                sound_primitives.insert((entry.hanzi().as_char(), entry.code().to_string()));
                let candidate = (
                    entry.is_official(),
                    entry.frequency_score(),
                    entry.code().to_string(),
                );
                preferred
                    .entry(entry.hanzi().as_char())
                    .and_modify(|known| {
                        if (candidate.0 && !known.0)
                            || (candidate.0 == known.0 && candidate.1 > known.1)
                            || (candidate.0 == known.0
                                && candidate.1 == known.1
                                && candidate.2 < known.2)
                        {
                            *known = candidate.clone();
                        }
                    })
                    .or_insert(candidate);
            }
        }
        for entry in canonical_word_code_entries() {
            static_exact.insert((entry.word().to_string(), entry.code().to_string()));
        }
        for entry in canonical_primary_shortcut_entries() {
            static_exact.insert((entry.word().to_string(), entry.shortcut_code().to_string()));
        }
        for entry in canonical_fixed_first_shortcut_entries() {
            static_exact.insert((entry.word().to_string(), entry.shortcut_code().to_string()));
        }
        let extended_exact = canonical_extended_word_code_entries()
            .iter()
            .map(|entry| (entry.word().to_string(), entry.code().to_string()))
            .collect();
        Index {
            static_exact,
            extended_exact,
            sound_primitives,
            preferred_sound: preferred
                .into_iter()
                .map(|(character, (_, _, code))| (character, code))
                .collect(),
        }
    })
}

/// 为文本机械选择每字的首选两键原语并拼接；任一字符没有输入事实则返回
/// `None`。该函数不查询词库，也不访问网络。
pub fn preferred_open_composition_code(text: &str) -> Option<String> {
    let mut code = String::with_capacity(text.chars().count() * 2);
    for character in text.chars() {
        code.push_str(index().preferred_sound.get(&character)?);
    }
    (!code.is_empty()).then_some(code)
}

/// 分类一个文本/精确输入码的可达路径；纯内存、离线、确定性。
pub fn classify_reachability(text: &str, code: &str) -> Reachability {
    let index = index();
    let key = (text.to_string(), code.to_string());
    let chars: Vec<char> = text.chars().collect();
    let bytes = code.as_bytes();
    let structurally_composable = chars.len() >= 2
        && bytes.len() == chars.len() * 2
        && bytes.iter().all(u8::is_ascii_lowercase)
        && chars.iter().enumerate().all(|(position, &character)| {
            let start = position * 2;
            let sound = &code[start..start + 2];
            index
                .sound_primitives
                .contains(&(character, sound.to_string()))
        });
    Reachability {
        static_reachable: index.static_exact.contains(&key),
        extended_lexicon_reachable: index.extended_exact.contains(&key),
        open_composition_reachable: structurally_composable,
        sentence_reachable: structurally_composable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_phrase_below_old_top_n_is_reachable_in_extended_tier() {
        assert_eq!(
            preferred_open_composition_code("提示词").as_deref(),
            Some("tiuici")
        );
        let result = classify_reachability("提示词", "tiuici");
        assert!(!result.static_reachable);
        assert!(result.extended_lexicon_reachable);
        assert!(result.open_composition_reachable);
        assert!(result.sentence_reachable);
        assert!(result.is_reachable());
    }

    #[test]
    fn realistic_sentences_have_mechanical_open_codes() {
        for (text, expected_code) in [
            ("嗯我觉得这样就可以了", "enwojtdeveyhjqkeyile"),
            ("诶你怎么也在这里", "einizfmeyezdveli"),
            (
                "我觉得这个输入法现在好多了呢",
                "wojtdevegeuurufaxmzdhcdolene",
            ),
            ("这个提示词应该没有什么问题", "vegetiuiciykgdmwyzufmewfti"),
            ("我今天准备继续完善这个项目", "wojbtmvybwjixuwjujvegexlmu"),
        ] {
            let code = preferred_open_composition_code(text)
                .unwrap_or_else(|| panic!("真实句子每字都应有 sound primitive: {text}"));
            assert_eq!(code, expected_code, "runtime fixture 必须由同一算法派生");
            let result = classify_reachability(text, &code);
            assert!(result.open_composition_reachable, "{text} {code}");
            assert!(result.sentence_reachable, "{text} {code}");
        }
    }

    #[test]
    fn arbitrary_attested_character_composition_is_open() {
        let result = classify_reachability("提嗯诶", "tiogei");
        assert!(!result.static_reachable);
        assert!(!result.extended_lexicon_reachable);
        assert!(result.open_composition_reachable);
    }

    #[test]
    fn extended_lexicon_is_an_acceleration_not_a_boundary() {
        let entry = canonical_extended_word_code_entries()
            .first()
            .expect("扩展词层非空");
        let result = classify_reachability(entry.word(), &entry.code().to_string());
        assert!(!result.static_reachable);
        assert!(result.extended_lexicon_reachable);
        assert!(result.open_composition_reachable);
    }

    #[test]
    fn malformed_or_unknown_code_is_unreachable() {
        assert!(!classify_reachability("提示词", "tiuic1").is_reachable());
        assert!(!classify_reachability("😀", "ti").is_reachable());
    }
}
