//! Small, deterministic lexical/context projection; no runtime corpus parser.

use std::collections::BTreeMap;
use std::fmt::Write;

use xhup_decoder::{BaselineScorer, BigramModel};

use crate::char_codes::finalized_char_code_entries;
use crate::lua_build::{LuaBuildError, sha256};

const CONTEXT: &str = include_str!("../../../data/corpus/kdconv_bigram.tsv");
const MAX_PAIRS: usize = 16_384;

fn quoted(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' | '\\' => {
                out.push('\\');
                out.push(character);
            }
            '\0'..='\u{1f}' | '\u{7f}' => {
                write!(out, "\\{:03}", u32::from(character)).unwrap();
            }
            _ => out.push(character),
        }
    }
    out.push('"');
}

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
    let mut out = String::new();
    writeln!(
        out,
        "local model = {{ policy = \"joint-frequency-context-v2\", total_reward = {denominator},"
    )
    .unwrap();
    writeln!(
        out,
        "  context_source = \"data/corpus/kdconv_bigram.tsv\", context_sha256 = \"{}\",",
        sha256(CONTEXT.as_bytes())
    )
    .unwrap();
    writeln!(out, "  context_pairs = {}, characters = {{", pairs.len()).unwrap();
    for (sound, characters) in readings {
        writeln!(out, "    [\"{}{}\"] = {{", sound[0], sound[1]).unwrap();
        for (character, frequency) in characters {
            out.push_str("      [");
            quoted(&mut out, character.encode_utf8(&mut [0; 4]));
            writeln!(
                out,
                "] = {},",
                BaselineScorer::frequency_reward(frequency) - denominator
            )
            .unwrap();
        }
        out.push_str("    },\n");
    }
    out.push_str("  }, transitions = {\n");
    let mut previous = None;
    for (left, right, count) in pairs {
        if previous != Some(left) {
            if previous.is_some() {
                out.push_str("    },\n");
            }
            out.push_str("    [");
            quoted(&mut out, left);
            out.push_str("] = {\n");
            previous = Some(left);
        }
        out.push_str("      [");
        quoted(&mut out, right);
        // Conservative, bounded count evidence, not a calibrated conditional
        // language probability. Parameters are fixed before new dev evaluation.
        writeln!(
            out,
            "] = {},",
            (BaselineScorer::frequency_reward(count) / 4).min(2048)
        )
        .unwrap();
    }
    if previous.is_some() {
        out.push_str("    },\n");
    }
    out.push_str("  },\n}\n");
    let identity = sha256(out.as_bytes());
    writeln!(out, "model.identity = \"{identity}\"\nreturn model").unwrap();
    Ok((out, identity))
}
