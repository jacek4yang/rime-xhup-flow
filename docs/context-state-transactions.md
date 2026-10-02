# Rust contextual snapshot transactions (F04/F09 supporting tier)

This module manages only `xhup_flow_user_model.tsv`; it is separate from native
Rime userdb and the optional Lua observer. No claim of unified runtime learning
is made by hardening this research/context store.

Save/import/export/reset now acquire a stable `.xhup-flow-context.lock` (never
unlink on unlock), reject ambiguous or foreign existing snapshots, and refuse
symbolic links. Import reads at most 16 MiB and validates before touching the
destination. Read-only load sees either old/new atomic bytes and does not create
a lock file. Schema-valid UTF-8 corruption may still return the documented empty
model with a degradation reason; filesystem/size/encoding failures are explicit.

Temporary files are exclusively created with unique per-process sequence names
and 0600 POSIX permissions. Write and file sync errors propagate; failed replace
preserves the old destination and cleans only that operation's temporary file.
Export uses the same replacement mechanism and will not truncate an unrelated
output. Source/destination export locks are nonblocking, preventing deadlock.
Reset requires confirmation AND a validated owned schema. Invalid files are not
automatically erased just because their external filename looks right.

No directory fsync or physical power-loss guarantee. This is a cooperative
same-user management contract, not a sandbox against a hostile process with the
same user's file access. Interrupted operations can leave owned temporary files;
ambiguous preexisting files are never silently removed.

Tests cover busy-store refusal across all four operations, foreign/linked
snapshots, preservation of preexisting legacy temp files, replacement failure,
output ownership, legitimate migration and round trips, and diagnostic privacy.
