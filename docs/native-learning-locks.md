# Native learning management safety (F09 native tier)

Scope: `learning export/import/reset` manages **only** `xhup_flow_user.userdb`.
The Rust contextual-memory file and optional Lua observer snapshot are separate
stores. These operations do not reset them or imply their concurrency is solved.
Unified contextual storage/learning remains a GA gate.

Management clients serialize using a stable `.xhup-flow-learning.lock` file.
The file is never unlinked on unlock (avoids competing lock inodes). Read-only
status does not acquire this management lock.

The native database must be inactive. POSIX uses the same **fcntl record lock**
as LevelDB (NOT flock). Windows uses overlapping LockFileEx ranges through Rust's
file-lock API. Native LOCK remains held across copying/inspection/export/reset.
Symlinks, unexpected directory/files, missing CURRENT, unknown database files,
over 4096 files or over 256 MiB of native files are refused rather than guessed.
These are management-time envelopes, not a user-state retention policy.

The original is never opened by librime merely to inspect ownership: under its
lock, recognized regular LevelDB files are copied into an exclusive private
workspace (0700 POSIX). Original LOCK is not reopened/copied: on POSIX closing a
second descriptor for that inode would release process fcntl locks. Native Rime
opens ONLY the copy; its snapshot must pass identity/type/version validation.
A foreign DB renamed to the Flow filename is rejected byte-unchanged.

- **Export:** preserves original native bytes. Creates private staged output in
  the destination filesystem, checks existing output ownership, syncs file bytes
  and renames. No parent-directory fsync/power-loss guarantee.
- **Import:** validates snapshot and existing DB identity; releases inspection's
  native lock so the manager can acquire its own lock. A competing native session
  may cause failure, never permits bypass. Management lock remains held.
  Preexisting native `.temp.userdb` is never removed by our command.
- **Reset:** while holding native LOCK, renames the verified DB to its private
  workspace; closes the handle then removes only that quarantined path. A newly
  created DB at the original path is untouched. Removal failure returns its path
  and preserves the workspace for recovery rather than claiming success.

The process/user and selected native executable are trusted. This is not a
sandbox against a hostile process running as the same OS user. Interruptions may
leave private workspaces containing user data; never attach them to diagnostics
or releases. No runtime network is used.

Evidence: real native import/export/historical migration, renamed foreign DB
byte preservation, symlink refusal, output ownership, cross-process POSIX fcntl
activity, reset and contextual-sentinel preservation. Generic operation lock
unit tests run cross-platform. The POSIX test is not Windows/macOS frontend
acceptance; final native platform qualification remains required.
