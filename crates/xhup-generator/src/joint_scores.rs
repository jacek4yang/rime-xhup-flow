//! Small, deterministic lexical/context projection; no runtime corpus parser.

use std::collections::BTreeMap;
use std::fmt::Write;

use xhup_decoder::{BaselineScorer, BigramModel};

use crate::char_codes::finalized_char_code_entries;
use crate::lua_build::{LuaBuildError, sha256};

const CONTEXT: &str = include_str!("../../../data/corpus/kdconv_bigram.tsv");
const MAX_PAIRS: usize = 16_384;

pub(crate) fn generate(flow: &str) -> Result<(String, String), LuaBuildError> {
    let error = |message| LuaBuildError::new("joint_scores", message);
    let mut characters = BTreeMap::<char, u64>::new();
    let mut readings = BTreeMap::new();
    for entry in finalized_char_code_entries() {
        let frequency = u64::from(
            u32::try_from(entry.frequency_score())
                .unwrap_or(u32::MAX / 2)
                .max(1),
        );
        characters
            .entry(entry.hanzi().as_char())
            .and_modify(|old| *old = (*old).max(frequency))
            .or_insert(frequency);
        readings
            .entry(&entry.code().as_slice()[..2])
            .or_insert_with(BTreeMap::<char, u64>::new)
            .entry(entry.hanzi().as_char())
            .and_modify(|old| *old = (*old).max(frequency))
            .or_insert(frequency);
    }
    // Count each text once, not once per spelling or pronunciation. Borrow the
    // actual generated rows so scoring cannot silently use a different lexicon.
    let mut words = BTreeMap::<&str, u64>::new();
    let (_, rows) = flow
        .split_once("\n...\n")
        .ok_or_else(|| error("missing dictionary body"))?;
    for row in rows.lines() {
        let mut fields = row.split('\t');
        let text = fields.next().ok_or_else(|| error("missing text"))?;
        let _code = fields.next().ok_or_else(|| error("missing code"))?;
        let weight: u64 = fields
            .next()
            .ok_or_else(|| error("missing weight"))?
            .parse()
            .map_err(|_| error("invalid weight"))?;
        if text.chars().nth(1).is_some() {
            words
                .entry(text)
                .and_modify(|old| *old = (*old).max(weight))
                .or_insert(weight);
        }
    }
    let total = characters
        .values()
        .chain(words.values())
        .try_fold(0_u64, |sum, count| sum.checked_add(*count))
        .ok_or_else(|| error("frequency total overflow"))?;
    drop(words);
    let denominator = BaselineScorer::frequency_reward(total);
    let model = BigramModel::from_tsv(CONTEXT)
        .map_err(|message| LuaBuildError::new("joint_scores", message))?;
    let mut pairs: Vec<_> = model.transitions().collect();
    if pairs.len() > MAX_PAIRS {
        pairs.select_nth_unstable_by(MAX_PAIRS, |a, b| {
            b.2.cmp(&a.2)
                .then_with(|| a.0.cmp(b.0))
                .then_with(|| a.1.cmp(b.1))
        });
        pairs.truncate(MAX_PAIRS);
    }
    pairs.sort_unstable_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    // Compatibility heuristic for discarding two explicitly typed shape keys.
    // Use the same integer Q10 approximation as lexical frequencies, not a
    // claimed calibrated probability. Negative runtime costs keep user rewards
    // on their existing native probability scale.
    let split_penalty = BaselineScorer::frequency_reward(26 * 26 - 1);
    let mut out = String::new();
    writeln!(
        out,
        "local model = {{ policy = \"joint-frequency-context-shape-v3\", total_reward = {denominator}, primary_full_code_split_penalty = {split_penalty},"
    )
    .unwrap();
    writeln!(
        out,
        "  context_source = \"data/corpus/kdconv_bigram.tsv\", context_sha256 = \"{}\",",
        sha256(CONTEXT.as_bytes())
    )
    .unwrap();
    writeln!(
        out,
        "  context_pairs = {},",
        pairs.len()
    )
    .unwrap();
    // Packed projections: "\003 key \002 item \001 item ..." sections. The
    // runtime decodes a row lazily on first access and memoizes it; values
    // are identical to the historical table form. The packed form keeps the
    // module load at string-literal cost (table-constructor execution of the
    // historical form measured 21.8 ms per process) and avoids resident Lua
    // hash tables for rows a session never queries.
    let mut characters_packed = String::new();
    for (sound, characters) in readings {
        characters_packed.push_str("\\003");
        let mut sound_key = String::new();
        for character in sound.iter() {
            sound_key.push((*character).into());
        }
        escaped(&mut characters_packed, &sound_key);
        characters_packed.push_str("\\002");
        for (character, frequency) in characters {
            escaped(&mut characters_packed, character.encode_utf8(&mut [0; 4]));
            characters_packed.push('|');
            let reward = BaselineScorer::frequency_reward(frequency) - denominator;
            write!(characters_packed, "{reward}").unwrap();
            characters_packed.push_str("\\001");
        }
    }
    writeln!(
        out,
        "  characters_packed = \"{}\",",
        characters_packed
    )
    .unwrap();
    let mut transitions_packed = String::new();
    for (left, right, count) in pairs {
        transitions_packed.push_str("\\003");
        escaped(&mut transitions_packed, left);
        transitions_packed.push_str("\\002");
        // Conservative, bounded count evidence, not a calibrated conditional
        // language probability. Parameters are fixed before new dev evaluation.
        escaped(&mut transitions_packed, right);
        let reward = (BaselineScorer::frequency_reward(count) / 4).min(2048);
        write!(transitions_packed, "|{reward}\\001").unwrap();
    }
    writeln!(
        out,
        "  transitions_packed = \"{}\",",
        transitions_packed
    )
    .unwrap();
    out.push_str("}\n");
    let identity = sha256(out.as_bytes());
    writeln!(out, "model.identity = \"{identity}\"\nreturn model").unwrap();
    Ok((out, identity))
}

/// Escape a packed-projection fragment into a portable Lua string literal.
/// Control bytes (including the \001/\002/\003 delimiters) always emit as
/// three-digit decimal escapes so the delimiter bytes stay unambiguous.
fn escaped(out: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\0'..='\u{1f}' | '\u{7f}' => {
                write!(out, "\\{:03}", u32::from(character)).unwrap();
            }
            _ => out.push(character),
        }
    }
}
