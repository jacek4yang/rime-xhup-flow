//! Deterministic in-sample scale/ablation report, NOT independent quality evidence.
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use xhup_analyzer::multi_source_evidence::{
    DailyPriorWeights, EvidenceSourceId, build_from_canonical,
};

fn distribution(mut values: Vec<f64>) -> Value {
    values.retain(|v| v.is_finite());
    values.sort_by(f64::total_cmp);
    if values.is_empty() {
        return json!({"observed": 0});
    }
    let at = |q: f64| values[((values.len() - 1) as f64 * q).round() as usize];
    json!({"observed": values.len(), "min": at(0.0), "p50": at(0.5),
        "p95": at(0.95), "max": at(1.0)})
}

fn top_words(scores: &BTreeMap<String, f64>) -> BTreeSet<String> {
    let mut entries: Vec<_> = scores.iter().collect();
    entries.sort_by(|a, b| b.1.total_cmp(a.1).then(a.0.cmp(b.0)));
    entries
        .into_iter()
        .take(1000)
        .map(|(w, _)| w.clone())
        .collect()
}

fn main() {
    let data = xhup_analyzer::build_analysis();
    let words = data
        .words
        .iter()
        .map(|entry| {
            (
                entry.word().to_string(),
                data.frequency.word_probability(entry.frequency_score()),
            )
        })
        .collect();
    let set = build_from_canonical(&words);
    let mut sources = BTreeMap::new();
    for source in EvidenceSourceId::ALL {
        let raw = set
            .entries()
            .iter()
            .filter_map(|e| e.signal(source))
            .collect();
        let normalized: Vec<_> = set
            .entries()
            .iter()
            .filter_map(|e| e.normalized_signal(source))
            .collect();
        assert!(
            normalized
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        );
        sources.insert(
            source.as_str(),
            json!({"raw": distribution(raw),
            "normalized": distribution(normalized), "total": set.entries().len()}),
        );
    }
    let score = |weights: DailyPriorWeights| -> BTreeMap<String, f64> {
        set.entries()
            .iter()
            .map(|e| {
                let prior = set.daily_prior(e, &weights);
                assert!(prior.is_finite() && (0.0..=1.0).contains(&prior));
                (e.word().to_string(), prior)
            })
            .collect()
    };
    let baseline = score(DailyPriorWeights::default());
    let baseline_top = top_words(&baseline);
    let mut ablations = BTreeMap::new();
    for (name, weights) in [
        (
            "lexical_only",
            DailyPriorWeights {
                wanxiang: 1.0,
                conversation: 0.0,
                kdconv_ge2: 0.0,
                sogou_sys_freq: 0.0,
            },
        ),
        (
            "equal_domains",
            DailyPriorWeights {
                wanxiang: 1.0,
                conversation: 1.0,
                kdconv_ge2: 0.0,
                sogou_sys_freq: 0.0,
            },
        ),
        (
            "conversation_only_with_missing_lexical_fallback",
            DailyPriorWeights {
                wanxiang: 0.0,
                conversation: 1.0,
                kdconv_ge2: 0.0,
                sogou_sys_freq: 0.0,
            },
        ),
        (
            "binary_channels_removed",
            DailyPriorWeights {
                wanxiang: 0.6,
                conversation: 0.2,
                kdconv_ge2: 0.0,
                sogou_sys_freq: 0.0,
            },
        ),
    ] {
        let scores = score(weights);
        let deltas: Vec<_> = scores
            .iter()
            .map(|(w, v)| (v - baseline[w]).abs())
            .collect();
        ablations.insert(name, json!({"distribution": distribution(scores.values().copied().collect()),
            "absolute_delta": distribution(deltas), "top1000_overlap_default": top_words(&scores).intersection(&baseline_top).count()}));
    }
    let report = json!({
        "model": "source-midrank-domain-dedup-v1",
        "scope": "in-sample deterministic scale audit; not held-out quality or calibrated usage probability",
        "default_nominal_weights": {"wanxiang": 0.6, "conversation": 0.2, "kdconv_ge2": 0.1, "sogou_sys_freq": 0.1},
        "sources": sources, "default_utility": distribution(baseline.values().copied().collect()),
        "ablations": ablations
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("finite audit report")
    );
}
