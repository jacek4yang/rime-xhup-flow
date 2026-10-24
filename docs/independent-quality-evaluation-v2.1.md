# Independent mixed-code quality evaluation — v2.1 joint search (2026-10-25)

Paired frozen-corpus evaluation of the bounded joint decoder (default OFF)
versus the native planner, on the current consolidation tree
(`84680e84118c33f9381d5de36417b31fa41fc420`, content-identical to main
`74dd0f6` for the runtime surface). This report is observational evidence for
the default-on decision; it is NOT a performance qualification and NOT GUI
acceptance.

## Provenance (fail-closed, all hashes verified before the run)

- Corpus: UD_Chinese-PUD `zh_pud-ud-test.conllu`, revision
  `2849afd946a8c01b3e9acdf3e7afa8670cf2777d`, source sha256
  `e12582af…` and fixture/partition sha256s match `tests/quality/pud-legal-source.json`
  byte-for-byte (development 519 cases / `69a893fd…`, confirmation 531 cases /
  `13782e3e…`). All five frozen dictionaries in the generated package match the
  manifest sha256s, so the frozen fixture remains valid for this tree.
- Harness: `run_native_corpus.py` self-verifies that every harness source
  matches its committed blob before running (LF clone; working tree == HEAD).
- Runtime: WSL Ubuntu 26.04, librime 1.16.1, pinned-source librime-lua
  `68f9c36` (built per `tests/librime/build-supported-lua.sh`), learning off,
  five-item menu, joint decoder toggled via `XHUP_JOINT_DECODER`.
- No tuning was performed against the confirmation partition before or during
  this report; it had not been replayed since the #229/#230 freeze.

## Confirmation partition (untouched; 177 cases per mode)

| mode | metric | joint OFF | joint ON | delta |
|---|---|---|---|---|
| full-code | top-1 | 31.6% | **78.5%** | +46.9 pp |
| full-code | top-3 | 31.6% | 94.4% | +62.7 pp |
| full-code | first page | 31.6% | 96.6% | +65.0 pp |
| full-code | exact commit | 32.2% | **98.3%** | +66.1 pp |
| alternating | top-1 | 34.5% | 66.7% | +32.2 pp |
| alternating | exact commit | 38.4% | 88.1% | +49.7 pp |
| shortest | top-1 | 34.5% | 54.2% | +19.8 pp |
| shortest | exact commit | 40.7% | 83.1% | +42.4 pp |

## Regressions (retained samples, not removed)

- `n01002017-run-13-shortest` (target 超大量, keys `icdal`): top-1 lost with
  joint ON (native had it first); exact commit still succeeds on both.
- `n01149002-run-1070-shortest` (target 埋在, keys `mdz`): first-page lost with
  joint ON; exact commit still succeeds on both.

Both are ranking-position regressions in shortest-alias mode, not commit
failures: 2 of 531 confirmation cases (0.38%). The development partition
(519 cases, observed during #229/#230 tuning) shows the same gains with zero
regressions; its numbers are therefore not independent evidence and are
recorded in the archived summaries only.

## Boundaries

- Latency numbers from these runs are observational (edit probes under
  concurrent replay), not paired performance qualification (#225 remains
  open); joint search stays default-OFF until that gate passes.
- Real-frontend acceptance (#150/#151/#219/#223 GTK/Weasel) is separate; the
  runtime-level #223 six-case contract passed on librime 1.16.1 in the joint
  regression.
- `contract_failures: 0` in every cell: no invalid spans, no duplicate
  candidates, no lost native fallback contract violations in either mode.

Raw summaries: `summary-confirmation.json` / `summary-development.json`
(WSL audit environment); reproduction commands recorded in the handoff.
