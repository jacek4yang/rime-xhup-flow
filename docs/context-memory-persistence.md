# Contextual snapshot safety (F04)

The optional Lua observer is **not a replacement for native Rime userdb**.
Its current safety contract is intentionally narrower than full cross-platform
persistent learning; completing the unified learning contract remains a GA gate.

- A write never removes the previous snapshot before replacement.
- POSIX Lua `os.tmpname()` reserves a temporary file using `mkstemp`; the module
  checks write, flush and close before `rename`. It cleans the reserved file on
  failure. This assumes a trusted local process/user and standard POSIX Lua.
- `rename` failure (including permissions, destination directory, or different
  filesystems between the OS temporary directory and user directory) preserves
  the old snapshot. There is no unsafe copy/delete fallback.
- Windows standard Lua lacks exclusive temporary creation and atomic overwrite
  APIs. This implementation explicitly refuses persistence there; it does NOT
  claim successful cross-platform durable learning. A native storage integration
  or elimination of this separate store is required before GA qualification.
- Failed writes return an error, retain pending observations and dirty state, and
  expose `env.persistence_error`; another `flush` can retry.
- Normal component finalization disconnects the commit observer and attempts a
  final flush. Failure is logged without observed text and returned; dirty state
  is not falsely cleared. Process termination cannot guarantee finalization.
- `flush`/`close` do not imply `fsync`, directory sync, power-loss protection or a
  maximum amount of loss following repeated failures.

Tests: `tests/lua/test_user_memory.lua` checks filesystem round trips;
`tests/lua/test_user_memory_failures.lua` injects create/open/write/flush/close/
replace/permission failures, checks retry/finalization, and checks a conflicting
real destination. Stubbed failures are not native Windows acceptance or physical
crash testing. CWD path selection, bounds and shared-state lifecycle still belong
to the pending unified-learning work.
