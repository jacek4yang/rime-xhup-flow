> 历史设计记录：当前 Trainer 已移除输入法管理及内嵌安装包，本文描述的安装/导出功能不再提供。当前安装方式见[手动教程](install-guide.zh-CN.md)。

# Installer transaction safety — current hardening (F15)

The existing plan/apply design remains. Apply now resolves the user root, holds
a stable installer lock, rejects duplicate targets, checks every package and
backup parent for links/non-directories, and refuses stale Write plans if their
destination appeared after planning. Backup targets must be regular files.

Staging uses `create_new`, never truncates a preexisting file/link, checks write
and file sync, and removes only its own failed staging file. Commit still uses
same-directory rename. On commit failure all prior actions are rolled back in
reverse order; one failed restoration does not prevent the remaining attempts.
A rollback failure is returned alongside the original failure, not silently
ignored. Remaining temporary paths from that attempt are cleaned where possible.
The shared lock inode is never removed at unlock.

Tests cover preexisting staging files, serialization, duplicate/stale plans,
linked package/backup parents, continuing rollback after one restoration fails,
and the existing install/upgrade/repair/uninstall and unrelated-file preservation
suite. Target-platform tests remain separate from these filesystem tests.

Limits: this is not a generic package manager or a crash-recovery journal.
Interrupted operations can leave staging/backup state requiring inspection;
there is no automatic deletion of ambiguous preexisting temporary files.
Canonical package-file ownership is still the existing exact owned-file manifest;
a persistent per-install provenance journal and historical adoption semantics
remain part of the final installer review. Against a hostile process running as
the same user, path checks plus cooperative locks are not an openat-style sandbox.
No parent-directory fsync or physical power-loss durability is claimed.
