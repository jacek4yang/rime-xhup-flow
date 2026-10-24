# Joint decoder paired performance qualification (2026-10-25)

Paired native-API measurement of the bounded joint decoder ON (candidate)
versus OFF (baseline). Methodology matches issue #225: 5 alternating AB/BA
pairs x 10 repetitions per side, CPU pinned to core 0 and core 8, learning
off and on — 4 groups, 19,800 key events per side per group, five-item
menu, learning state fresh per process. Native API key-to-menu latency is
NOT GUI latency; both sides measured identically.

Thresholds: per-scenario p95/p99 (keypress, key-to-menu, CPU-time), peak
RSS and first-session median/max — candidate/baseline ratio must not exceed
1.10. No threshold was changed.

## Verdict after optimization round 1: still FAIL — joint decoder stays default-OFF

Two optimization rounds against the profiled leads (both verified):

1. Baseline measurement (tree `84680e84` = main `74dd0f6`): 225/288 latency
   ratios over budget; key-to-menu p95/p99 1.6-1.8x; single-scenario p99
   ~4.6-5.4x; peak RSS 1.35-1.37x (+12.6 MiB). Instrumentation of the first
   joint query attributed 21.8 of the 22.6 ms warm-up to loading the joint
   score model (`require("xhup_flow.data.joint_scores")` — 736 KB of Lua
   table constructors executed per process).
2. Optimization (branch `perf/joint-scores-compact`): the generator now
   emits the score projections as packed control-delimited strings decoded
   lazily with memoization (values identical; module shrinks 736 KB -> 578
   KB). Re-measured on the same protocol:
   - scorer load **21.8 ms -> 2.85 ms**; single-scenario p99 ratio
     **5.3x -> 1.8x** (the warm-up spike is gone);
   - peak RSS ratio **1.35-1.37x -> 1.29-1.30x** (+12.6 -> +10.2 MiB);
   - key-to-menu p95/p99 **1.6-1.8x -> 1.35-1.69x**;
   - first session 0.95-1.11x (one group's median 1.106 marginally over);
   - semantic differences identical to the pre-optimization run (18,600
     rows per group) plus joint regression, exhaustive alias-projection
     audit and Lua source/compiled suites all PASS — the change is
     representation-only.
   - Latency: still 216/288 ratios over budget. The remaining overage is
     the steady-state per-keystroke cost of the bounded beam search and
     native row materialization (structural, not warm-up). Closing it needs
     either an incremental lattice that reuses the previous keystroke's
     search, a native (C) implementation, or a budget decision specific to
     feature-on-vs-feature-off comparisons; none of these are claimed here.

## Verdict (round 0, pre-optimization; retained for the record)

- **Latency: 225 of 288 ratios exceed 1.10** across all four CPU/learning
  groups. Key-to-menu p95/p99 ratios are typically **1.6–1.8x** (e.g. cpu0/
  learning-off aggregate p95 7.3ms -> 12.0ms). The `single` scenario's p99
  reaches **~4.6–5.4x** (4.3–4.9ms -> 22.3–24.4ms), indicating a first-use
  warm-up/lazy-initialization hot path on the joint search's first
  single-char query in a session.
- **Peak RSS: 1.35–1.37x in all four groups** (34.4–36.6 MiB -> 47.1–49.3
  MiB, +12.6 MiB) — the joint decoder's read-only projection/edge caches.
- **First session (median/max): 0.95–1.07x — within budget.**

Full per-scenario ratios are retained in the comparison outputs
(`perf-joint-cpu{0,8}-{off,on}/comparison.json`, WSL audit environment);
18,600 semantic difference rows per group are retained by policy (ranking
changes are the feature, not an error).

## Combined decision evidence for the default-on switch

| gate | result | evidence |
|---|---|---|
| Independent quality (frozen confirmation corpus) | large gains (exact commit 32.2% -> 98.3% full-code; top-1 +47pp; 2 retained regressions / 531) | `docs/independent-quality-evaluation-v2.1.md` |
| Paired performance (this report) | **FAIL** (latency p95/p99 1.6–1.8x, RSS 1.35x, single p99 ~5x) | this file |

Quality does not purchase a budget exception: the switch remains
default-OFF until the hot paths are optimized and this qualification is
re-run under the same protocol. Optimization leads from the data: (1) the
single-scenario p99 spike points at first-query warm-up; (2) the +12.6 MiB
RSS points at the joint read-only caches (projection reverse table +
canonical edge index materialization).

## Boundaries

- Not GUI/frontend latency; not Weasel/Squirrel/Fcitx acceptance.
- Not a cold-boot measurement (process restart with warm OS file cache).
- Issue #225's v2.0.0-vs-RC budget failures (single-char p99, first-session)
  were measured with joint OFF on both sides and remain open separately;
  this report isolates the joint-decoder increment only.
