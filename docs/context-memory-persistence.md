# Learning stores and session privacy (F04 / F08)

## Supported runtime contract

**Native Rime `xhup_flow_user` is the only durable runtime learning store.**
The native provider has a conservative persisted-tick update quota, preserves
genuine candidate identity, and reports learning refusal without stopping typing.
See [native-tail-investigation.md](native-tail-investigation.md) for its exact
bounds and [native-learning-locks.md](native-learning-locks.md) for
management ownership/locking (where applicable).

The historical `user_memory` option now means **session-only frequency evidence**
for the optional `context_ranker`. It does not enable/disable native learning:

- Both options off: do not even read committed text, load a file, or write a file.
- `context_ranker` on: keep only the latest permitted commit for repeat ranking.
- `user_memory` on: at most 512 distinct committed strings, each at most 256 bytes,
  with saturating counts at 65,535. A fixed FIFO ring bounds eviction work.
- Turning off session memory immediately clears its counts. Turning both options
  off clears the last commit too. Finalization disconnects observers and removes
  state. New sessions/processes start empty.
- State is keyed by a native context property, not one global text/count table;
  multiple engines cannot inherit or overwrite each other's evidence.

There is no second runtime TSV writer, CWD-dependent snapshot path, automatic
migration, or platform-specific Lua persistence fallback. Existing
`xhup_flow_user_model.tsv` files are left byte-for-byte untouched and are **not
read into runtime ranking**. Back them up explicitly if retaining research data.
The CLI `user-state` commands remain a separate offline/research format, not a
promise that the input method consumes those snapshots.

## Migration and tests

This deliberately retires the unsupported optional cross-platform Lua persistence
contract rather than pretending native learning and TSV counts are one model.
Native export/import/reset preserves its documented database ownership boundary;
it does not delete unrelated historical TSV files. Session evidence needs no
durable migration and never uploads text.

`tests/lua/test_session_memory.lua` tests bounds, consent before reading text,
immediate clearing, multi-engine isolation, finalization and zero file I/O with
stubs. `run-context-ranker-audit.sh` tests actual native sessions, exact commits,
ranking, restart-empty semantics, and an unchanged historical TSV sentinel.
`run-flow-audit.sh` separately tests actual persistent native learning and
management. These are distinct contracts, not interchangeable proof.

Historical snapshot replacement/failure tests are retained against
`tests/research/legacy_user_memory.lua`. That code is **not bundled or loaded by
Rime**; those tests must not be reported as current runtime persistence evidence.
No test here establishes power-loss or filesystem crash durability.
