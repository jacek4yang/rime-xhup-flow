# Evidence scale and research utility (F10 / F11)

Model: **source-midrank-domain-dedup-v1**. This is an offline research/audit
utility, not a calibrated usage probability, linguistic optimum, or a new
production mapping. Frozen canonical assignments and runtime dictionaries are
unchanged.

## Units and correlated sources

Raw Wanxiang probability and median-anchored log conversation measurements remain
available for explanation/classification. They are not directly added together.
For each numeric source, observed vocabulary values are mapped to empirical
midrank `(below + ties/2) / observed`, in [0,1]. Ties share one value; missing,
negative or non-finite input is absent, not a measured zero. A binary research
flag is explicit utility 1 when present, otherwise absent.

Before fusion, only the first measured source with a positive finite weight in
each declared domain is retained:

1. Lexical domain: Wanxiang, then Sogou-system research flag.
2. Conversation domain: conversation count, then its KDConv-derived binary flag.

This conservative grouping prevents duplicate evidence rewards; it does not
assert that different corpora are statistically independent. Default nominal
weights are 0.6/0.2/0.1/0.1; retained weights are renormalized per word. A numeric
source is preferred over its correlated flag regardless of the flag's weight.
Weights are scaled before summing to avoid overflow. With no usable weight,
Wanxiang's quantile is the explicit fallback; with no measurement, utility is 0.
Production clean-v1 does not embed either research-only binary whitelist.

Quantiles lose absolute probability ratios and depend on the measured vocabulary.
They correct the incompatible-scale arithmetic, not domain bias or missing-data
bias. Missing conversation evidence is not proof that a word is rare.

## Reproducible scale audit and ablations

```sh
cargo run --locked -p xhup-analyzer --bin evidence-scales-audit > /tmp/evidence.json
diff -u data/benchmarks/evidence-scales-v1.json /tmp/evidence.json
```

The report records all source coverage, raw and normalized distributions, and
source/weight ablations. On the committed 100,000-word research vocabulary:

- Conversation is measured for 23,813 words; binary production channels have
  zero observations.
- Wanxiang raw median is 0.0000026193, versus normalized median 0.499585.
  Conversation raw median is 0, versus normalized median 0.524545.
- Default utility median is 0.49525.
- Top-1000 overlap with the default is 466 for lexical-only, 860 for equal
  domain weights, and 656 for conversation-only with explicit missing-data
  lexical fallback. Removing unused binary channels leaves all results identical.

These large rank sensitivities are **not** evidence of better input quality.
The report is in-sample and must never be labeled held-out acceptance.
Regression tests cover ties, missing/invalid input, unit rescaling, duplicate
domain signals, binary ablation, extreme weights and bounded outputs.

## Downstream objective and acceptance boundary

Shortcut audit uses `exp(utility)` as an explicit positive soft-weight policy
([1,e] for this model), not as recovery of an underlying probability.
Generic caller-supplied utilities retain the existing finite-mass checks.

The v2 research optimizer combines a daily utility with conversation coverage and
diversity terms. These are correlated preferences, not independent evidence.
It also rewards keys saved and subtracts key cost: for a fixed full-code length,
this intentionally reinforces the same marginal preference (twice the key-cost
coefficient), rather than measuring two independent benefits. Its historical
sweeps compare policies under that chosen objective; they do not validate the
objective or demonstrate transfer to another population.

This change does not retune/promote a mapping. Promotion of any new research
weights or assignments requires source-disjoint, duplicate-controlled evaluation,
declared trade-offs, ablations and sentence/session-level uncertainty. Existing
KDConv replay is a regression suite, not that independent evaluation.
