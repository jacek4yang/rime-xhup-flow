# Release qualification evidence — 2026-10-25 (main `65c78c2`)

Consolidated gate evidence for the v2.1 release decision, all produced on the
current main with hash-bound packages. Environment: WSL Ubuntu 26.04,
librime 1.16.1, pinned-source librime-lua (`68f9c36`), GTK 3.24.52,
Fcitx5 5.1.19, Xvfb + private D-Bus. Raw outputs retained in the audit
environment (`~/xhup-audit`, reproduction commands in the handoff).

## 1. Release performance budget vs v2.0.0 (issue #225) — PASS

Protocol: `compare_runtime.py`, 5 AB/BA pairs x 10 repetitions per side,
CPU 0/8 x learning on/off, joint decoder OFF on both sides (schema default).

- **Semantic differences vs v2.0.0: 0 in every group** — the frozen static
  contract holds byte-for-byte through the release comparison.
- First session median/max: 0.85–1.06 in every run — **the #225 first-session
  blocker is resolved**. Peak RSS: 0.998–1.002 everywhere.
- p95: within the 1.10 budget in all clean runs.
- p99 at 5x10 sampling: 0–6 cells over budget per run with **scenario
  identity not reproducible across independent runs** (documented runs:
  perf3 [invalidated: concurrent package install during measurement],
  perf4, perf5). One cell (word-shortcut p99, cpu0/learning-on) exceeded in
  both perf4 and perf5 and was re-measured with 4x samples (10 pairs x 20
  repetitions, `perf6`): **0/72 ratios over budget, worst 1.027** — the
  5x10 p99 tail is noise-dominated on this shared host.
- Verdict: the like-for-like release budget holds; residual p99 tails at
  5x10 sampling are shared-host noise by reproducibility evidence, not a
  code regression. All raw data retained.

## 2. Real GTK/Fcitx frontend acceptance (`run_gtk_fcitx.py`) — #223 fixed via joint ON; #219 upstream-confirmed

| # | joint | control burst | delivery | result |
|---|---|---|---|---|
| 1 | ON | none | async (default) | **6/6 PASS** incl. `nihcnzqu`->你好去, `nihcnznihcnzqu`->你好你好去 |
| 2 | ON | cold (Esc/Ctrl+A/BS) | async | 5/6 — first composition lost (#219) |
| 3 | ON | buffered (…+q) | async | 5/6 — first `q` lost (#219) |
| 4 | ON | cold | `FCITX_ENABLE_SYNC_MODE=1` | **6/6 PASS** |
| 5 | OFF | none | async | 4/6 — `nihcnzqu`->你好耨去, `nihcnznihcnzqu`->你好耨你好耨去 (#223 retained on the native path) |

Findings:

- **#223 is fixed end-to-end through the real frontend when the joint
  decoder switch is enabled** — both original failures commit correctly,
  with real preedit, screenshots and no residual composition. The runtime
  contract previously proven at the librime level holds through GTK/Fcitx.
- With joint OFF (the shipping default) the #223 failures remain by design
  (the native planner is unchanged); continuous-input quality therefore
  ships as an opt-in switch whose qualification state is documented below.
- **#219 reproduces on current main** (first composition after a cold
  control-key burst is lost under async delivery) and is **upstream-attributed**:
  the identical scenario passes with `FCITX_ENABLE_SYNC_MODE=1`. This
  matches #219's original analysis (also present on the v2.0.0 baseline).
  Safe mitigation: document the environment variable; no Lua key-replay
  hack.

## 3. Joint decoder qualification state (default stays OFF)

- Quality (frozen confirmation corpus, untouched): exact commit 32.2% ->
  98.3% (full-code), top-1 +47pp; 2 retained ranking regressions / 531
  (`docs/independent-quality-evaluation-v2.1.md`).
- Performance after optimization round 1 (`#249`): warm-up 21.8 -> 2.85 ms,
  single p99 5.3x -> 1.8x, RSS 1.35x -> 1.29x — still FAIL vs the 1.10
  feature budget on steady-state per-keystroke cost (216/288 ratios;
  `docs/joint-decoder-performance-qualification.md`). The switch ships
  OFF; steady-state optimization (incremental lattice or native search)
  remains the follow-up.

## 4. Release decision inputs

| gate | state |
|---|---|
| Static compatibility vs v2.0.0 | identical (semantic diff 0 across all paired runs) |
| #225 release budget | PASS (deep-sampled; noise analysis retained) |
| #223 continuous input | fixed via opt-in joint switch; native default unchanged |
| #219 GTK cold start | open, upstream (GTK/Fcitx async); sync-mode mitigation documented |
| #150 Weasel trailing prefix | UNVERIFIED (no Weasel on this host) |
| Joint default-on | blocked on steady-state performance (documented) |
| Native learning / CLI | verified end-to-end (PR #248) |
| Trainer/miniapp | M1+M2 gates green (PRs #239-#246) |

GA remains blocked on #219 (upstream), #150 (Weasel verification) and the
default-off state of the continuous-input improvement. A truthful RC
refresh (2.1.0-rc.2) is supportable with this evidence; publishing requires
the owner's explicit authorization per the release policy.
