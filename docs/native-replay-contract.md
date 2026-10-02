# Native replay contract and qualification status

`tests/librime/runtime_replay.c` uses real librime and asserts Lua registration.
It runs only in an isolated generated-package deployment. It records each key,
raw input, composition/preedit, selected range, candidate texts, target/prefix
rank, keypress time, key-to-first-menu time (including `get_context`), and
bounded top-256 probe time. The replay fixture uses a five-candidate page. Search is explicitly bounded at 256 candidates; the first
16 menu entries are captured. Absence within that bound is not proof of absence
from every possible native path. Fixed public regression strings are not user
corpus data. No runtime telemetry is added. The isolated replay fixture disables
native learning in compiled `flow/enable_user_dict` and `learn/enable_user_dict`
configuration and asserts those values through the actual Rime API; a similarly
named context switch does NOT turn native learning off. `XHUP_REPLAY_VERIFY_LEARNING=1` additionally checks learning-off export,
recompiles with native learning enabled, checks native code identities in exported
records, and repeats the replay in a fresh process.

Run the hard gate:

```sh
XHUP_AUDIT_ONLY_REPLAY=1 tests/librime/run-flow-audit.sh PACKAGE_DIRECTORY
```

It returns failure if any contract fails. `XHUP_REPLAY_MODE=--observe` is an
investigation-only mode, explicitly labelled `OBSERVATION_NOT_ACCEPTANCE`; it
cannot be used as release evidence of passing regressions.

## Incomplete tails and ambiguity

Let `raw = decoded_prefix_keys + pending_suffix`. At an unfinished final code,
a valid previously decoded text prefix must still be represented by a candidate
within the supported top-256 envelope. A candidate may extend that prefix using
completion, or leave its pending suffix uncommitted. Raw keys must not be lost.
Top-1 may legitimately rerank; that alone is not a regression. No contract freezes
all historical main-scheme decisions.

- `jbzq` permits `jb+zq` and `jbz+q` ambiguity; it need not already show 进去 as
  top-1, but must retain a 进-prefix interpretation and the raw pending input.
- `jbzqu` must expose selectable 进去. Backspace restores the captured earlier
  state, extension reproduces it, and selection commits exactly the target.
- `nihcvegeuurufawojtdesuduhduikeyi` represents the public #150 sentence. Appending
  `d` must not erase its decoded prefix; backspace restores the captured state;
  appending `de` preserves/extends it.
- All 2/3/4 + 2/3/4 combinations are exercised with actual 你/好 codes. Fresh-session
  replay is deterministic. This nine-case matrix is a regression set, not broad
  independent quality evaluation.

Candidate text equality alone is insufficient: a partial candidate can consume
only part of the composition. Selection must commit the target exactly once and
consume the complete intended input. This distinction caught false reachability
in the existing native baseline.

## Initial findings (not acceptance)

The first clean-v1 native TableTranslator run (native learning still enabled in
the initial fixture, since corrected) failed 9 of 61 checks: missing
`jbzq` prefix, disrupted long pending tail, absent complete 3-key alternatives,
and a text-matching partial candidate that committed additional unmatched text.
The harness now additionally captures commit/remaining input and checks full
consumption, so subsequent totals will differ.

Pinned librime 1.16.1 source explains part of this behavior: TableTranslator
constructs overlapping lexical edges but Poet returns a single complete sentence,
requiring a path reaching the full input length. Prefix phrases subsequently
returned are not n-best complete paths. Full-input completion is not equivalent
to a decoded sentence followed by a pending suffix. Native ScriptTranslator and
minimal-policy alternatives must be measured before choosing a replacement.

This document is an executable test contract and investigation status, not a
claim that shipped production already satisfies it. Static compatibility remains
a separate frozen-contract tier.
