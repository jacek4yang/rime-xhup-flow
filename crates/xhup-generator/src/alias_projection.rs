//! Read-only alias normalization projection; never imported by the user dictionary.
//!
//! librime 1.16.1 de4700e9: ordered syllabary IDs, first duplicate wins,
//! stable raw-weight ordering, then float(log(weight)) - log(1e8).
//! The native audit compares every projected record against actual dictionaries.
//! Runtime additionally decodes every retained ID before using it for learning.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use crate::lua_build::{LuaBuildError, sha256};

pub(crate) const NAME: &str = "xhup_flow_alias_projection";
pub(crate) const FILENAME: &str = "xhup_flow_alias_projection.dict.yaml";
pub(crate) const IDENTITY_KEY: &str = "__xhup_alias_identity__";
const NORMALIZATION: f64 = 18.420_680_743_952_367;

struct Row<'a> {
    text: &'a str,
    code: &'a str,
    weight: u32,
    ordinal: usize,
}

fn rows(
    dictionary: &str,
) -> Result<impl Iterator<Item = Result<Row<'_>, LuaBuildError>>, LuaBuildError> {
    let (_, body) = dictionary
        .split_once("\n...\n")
        .ok_or_else(|| LuaBuildError::new(NAME, "missing dictionary body"))?;
    Ok(body
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut fields = line.split('\t');
            let text = fields.next().unwrap_or_default();
            let code = fields.next().unwrap_or_default();
            let weight = fields
                .next()
                .unwrap_or_default()
                .parse::<u32>()
                .map_err(|_| LuaBuildError::new(NAME, "expected resolved integer weight"))?;
            if text.is_empty()
                || code.is_empty()
                || !code.bytes().all(|byte| byte.is_ascii_lowercase())
            {
                return Err(LuaBuildError::new(
                    NAME,
                    "invalid text or canonical spelling",
                ));
            }
            Ok(Row {
                text,
                code,
                weight,
                ordinal: 0,
            })
        }))
}

/// Canonical sources are in librime import order: the owning table, then imports.
/// Include all static source tables, not the lossy quick-hint projection.
pub(crate) fn generate<'a>(
    canonical: [&'a str; 2],
    aliases: impl IntoIterator<Item = &'a str>,
) -> Result<(String, String), LuaBuildError> {
    let mut wanted = BTreeSet::new();
    for dictionary in aliases {
        for row in rows(dictionary)? {
            let text = row?.text;
            wanted.insert(text);
            for (start, character) in text.char_indices() {
                wanted.insert(&text[start..start + character.len_utf8()]);
            }
        }
    }
    let mut entries = Vec::new();
    for dictionary in canonical {
        for row in rows(dictionary)? {
            let mut row = row?;
            row.ordinal = entries.len();
            entries.push(row);
        }
    }
    entries.sort_unstable_by(|left, right| {
        left.code
            .cmp(right.code)
            .then(left.text.cmp(right.text))
            .then(left.ordinal.cmp(&right.ordinal))
    });
    entries.dedup_by(|later, first| later.code == first.code && later.text == first.text);
    let mut counts = BTreeMap::<&str, u8>::new();
    let mut body = format!(
        "# Generated read-only normalization metadata; not an input/learning dictionary.\n---\nname: {NAME}\nversion: \"{}\"\nsort: original\nuse_preset_vocabulary: false\n...\n",
        env!("CARGO_PKG_VERSION")
    );
    let body_start = body.len();
    let mut start = 0;
    let mut id = 0_i32;
    while start < entries.len() {
        let code = entries[start].code;
        let mut end = start + 1;
        while end < entries.len() && entries[end].code == code {
            end += 1;
        }
        let group = &mut entries[start..end];
        group.sort_unstable_by(|left, right| {
            right
                .weight
                .cmp(&left.weight)
                .then(left.ordinal.cmp(&right.ordinal))
        });
        for (rank, row) in group.iter().enumerate() {
            if !wanted.contains(row.text) {
                continue;
            }
            let count = counts.entry(row.text).or_default();
            if *count == 8 {
                continue;
            }
            *count += 1;
            // '!' sorts before a..z: prefix codes keep native reverse order.
            // Ineligible rows still consume the original normalization budget.
            if rank < 64 {
                let raw = if row.weight == 0 {
                    f64::EPSILON
                } else {
                    f64::from(row.weight)
                };
                let weight = f64::from(raw.ln() as f32) - NORMALIZATION;
                writeln!(body, "{}\t{code}!{id}!{weight}\t1", row.text).expect("String write");
            } else {
                writeln!(body, "{}\t{code}!x\t1", row.text).expect("String write");
            }
        }
        id = id
            .checked_add(1)
            .ok_or_else(|| LuaBuildError::new(NAME, "native syllabary exceeds i32"))?;
        start = end;
    }
    let identity = sha256(&body.as_bytes()[body_start..]);
    writeln!(body, "{IDENTITY_KEY}\t{identity}\t1").expect("String write");
    Ok((body, identity))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dictionary(body: &str) -> String {
        format!("---\n...\n{body}")
    }

    #[test]
    fn first_duplicate_and_native_ids_survive_projection() {
        let owning = dictionary("甲\tab\t2\n乙\tab\t2\n");
        let imported = dictionary("甲\tab\t999\n丙\taba\t10\n");
        let aliases = dictionary("甲乙\tx\t1\n丙\ty\t1\n");
        let (output, _) = generate([&owning, &imported], [&*aliases]).unwrap();
        let weight = f64::from(2_f64.ln() as f32) - NORMALIZATION;
        assert!(output.contains(&format!("甲\tab!0!{weight}\t1\n")));
        assert!(output.contains(&format!("乙\tab!0!{weight}\t1\n")));
        assert!(output.contains("丙\taba!1!"));
        assert!(!output.contains("甲乙\t")); // Unknown word uses its character rows.
    }

    #[test]
    fn row_cutoff_is_retained_as_a_budget_consuming_miss() {
        let mut canonical = String::new();
        for index in 0..65_u32 {
            writeln!(
                canonical,
                "{}\taa\t{}",
                char::from_u32(0x4e00 + index).unwrap(),
                65 - index
            )
            .unwrap();
        }
        let target = char::from_u32(0x4e00 + 64).unwrap();
        let aliases = dictionary(&format!("{target}\tx\t1\n"));
        let (output, _) =
            generate([&dictionary(&canonical), &dictionary("")], [&*aliases]).unwrap();
        assert!(output.contains(&format!("{target}\taa!x\t1\n")));
    }

    #[test]
    fn only_first_eight_reverse_codes_are_projected_in_prefix_order() {
        let mut canonical = String::new();
        for length in 2..11 {
            writeln!(canonical, "甲\t{}\t{length}", "a".repeat(length)).unwrap();
        }
        let aliases = dictionary("甲\tx\t1\n");
        let (output, _) =
            generate([&dictionary(&canonical), &dictionary("")], [&*aliases]).unwrap();
        assert!(output.contains("甲\taaaaaaaaa!7!"));
        assert!(!output.contains("甲\taaaaaaaaaa!"));
        let mut values: Vec<_> = output
            .lines()
            .filter(|line| line.starts_with("甲\t"))
            .map(|line| line.split('\t').nth(1).unwrap())
            .collect();
        let original = values.clone();
        values.sort_unstable();
        assert_eq!(values, original);
    }

    #[test]
    fn malformed_canonical_rows_fail_the_build() {
        let empty = dictionary("");
        for body in [
            "甲\ta b\t1\n",
            "甲\taa\t1%\n",
            "甲\tAA\t1\n",
            "甲\taa\t-1\n",
        ] {
            assert!(generate([&dictionary(body), &empty], [&*empty]).is_err());
        }
    }
}
