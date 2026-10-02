# Research decoder objective and bounds

`xhup-decoder` is an offline research library, not the shipped Rime runtime.
Its beam search now evaluates every prefix with the **same caller-supplied
scorer** used for final ranking. Previously, a fixed default lexical/segmentation
baseline discarded paths before a context/user/custom-penalty scorer saw them.
The scorer contract includes contiguous prefixes starting at input offset zero.

This fixes the objective mismatch, not the general limitations of beam search:
future context rewards need not be bounded by a prefix score. A narrow beam can
still discard the eventual optimum. `truncated`/`BeamTruncated` report this even
when no complete path survives. `fallback` is a diagnostic request to a caller,
not an implemented native/static runtime fallback. Do not advertise it as one.

Adaptive expansion is exact only when no hypotheses were truncated. The default
maximum width 32 is a research resource policy informed by historical single-token
menus, **not** proof of multi-token recall. Prefix rescoring costs path-length work
per expansion, rather than the former constant-time baseline increment; this is
not an IME latency optimization and must be measured before any runtime adoption.

`tests/pruning_objective.rs` uses an independently specified tiny graph:

- a context-like preference opposing frequency must survive width-one pruning;
- 24 combinations of penalties, rewards and delayed rewards compare untruncated
  adaptive results against complete enumeration, including exact paths/scores;
- a delayed-reward counterexample deliberately loses the optimum with width one
  and must report truncation instead of claiming exhaustive equivalence.

These are adversarial algorithm tests, not corpus-quality or platform acceptance.
