# Engineering handoff

Living handoff record for the v2.1 consolidation. Update after each milestone.
Free of secrets and personal data by policy.

## Snapshot

- Date: 2026-10-24.
- Main: `d567a95b8bdb24d42e4c49ff2cd8a5da89dd3260`.
- Remote branches: `main` only (verified with `git ls-remote --heads origin`).

## Consolidation record

- The v2.1 stacked draft chain (`#215` -> `#227` -> `#228` -> `#229` -> `#230`
  -> `#232` -> `#233`, plus the already-closed `#231` branch) was consolidated
  through PR `#234` (squash commit `d567a95`). Ancestry was verified with
  `git merge-base --is-ancestor` for every branch against the leaf `fab7844`;
  the squash tree is byte-identical to the verified branch head `3442bcb`.
- All seven stacked PRs are closed as superseded with an explicit reference to
  `d567a95`.
- Security fixes landed on main first: `#235` (tokio 1.54.1 for
  RUSTSEC-2026-0386) and `#236` (frontend dependency advisories). The braces
  advisory `GHSA-vfj7-8cjw-p6xm` was withdrawn upstream on 2026-10-23; the
  source-bound patch and its byte-identity verification remain in place.

## Verification evidence

- Local Windows (16-core host, cargo `-j 3`, <= 20% CPU budget):
  `cargo fmt --all -- --check`, `cargo check --workspace --all-targets
  --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --all-targets --locked` all PASS on the consolidated
  tree.
- Cloud CI run `40011282409` on tree-identical head `3442bcb`: Rust workspace,
  trainer frontend, librime smoke all PASS.
- WSL Ubuntu 26.04, librime 1.16.1, pinned-source librime-lua (`68f9c36`,
  built locally per `tests/librime/build-supported-lua.sh`):
  - alias projection audit PASS: 128,618 records over 108,208 static/character
    texts with exact IDs, weights, reverse order and 8/64 bounds.
  - joint regression PASS, including the issue `#223` six-case first-choice
    contract (`nihcnzqu` -> target as rank-0, exact commit, no duplicate).

## Findings recorded during integration

1. The audit fixtures failed silently: librime logs go to glog files, so a Lua
   init error left no trace on stderr. The alias probe now captures rime logs
   and the driver reports stderr plus log tails when the receipt is missing.
2. The generator's syllabary replication is pinned to librime 1.16.1
   (`de4700e9`, see `alias_projection.rs`). CI runs librime 1.10.0, where the
   syllabary numbering differs, so the exhaustive identity audits are
   version-scoped in `run-flow-audit.sh` (explicit SKIP with the reason below
   1.16). No assertion was weakened; on 1.16.x both audits run in full.
3. Upstream librime 1.16.1 defect: `rime_dict_manager -e` exports text-first
   records with packed values while `-r` restores code-first records with
   `c=/d=/t=` values; snapshots therefore do not round-trip (columns swap and
   commit counts reset). Legacy code-first snapshots restore faithfully. The
   product path `xhup-cli learning import` must be verified separately.
4. `ci.yml` Rust job timeout raised 45 -> 60 minutes: cold-cache rebuilds
   (cache keys changed by Cargo.lock updates, and pre-existing caches expired
   after 7 days) exceeded 45 minutes at the test stage.
5. `run_joint_regression.py` now takes snapshots via explicit-path `-e` exports
   (`-b` writes to `sync/<machine>/` on current librime) and its state parser
   accepts both the legacy `c=/d=/t=` and modern packed export formats.

## Pending work (priority order)

1. Behavior acceptance on real frontends for `#150`/`#151`/`#219`/`#223`. The
   runtime-level `#223` contract is proven on librime 1.16.1 (joint
   regression); the GTK3/Fcitx cold-start issue `#219` remains open, and real
   Weasel/GTK platform runs are pending.
2. Independent quality evaluation (frozen `#229` dataset) and performance
   qualification (`#225`) before any decision to enable `joint_decoder` by
   default; the switch ships default-off.
3. Trainer, miniapp, CLI validation; full regression gate on final main
   (dispatched as run `40016903139`); reproducible packaging and artifact
   hashes.
4. Platform acceptance matrix and the release-readiness decision. 2.1.0 GA
   stays blocked while `#219`/`#223` platform evidence and `#225` performance
   gates are incomplete.
5. Local branch hygiene: 74 local branches with `[gone]` upstreams are kept
   for now; several contain pre-squash-era commits whose content is in main
   but whose SHAs are not ancestors. Audit before deleting.

## Environment notes

- The gh token now carries the `workflow` scope (required to push workflow
  file changes; granted via device-code flow on 2026-10-24).
- The WSL 1.16.1 audit environment lives under `~/xhup-audit` (generated
  package copy, pinned lua plugin build, `tests/librime` scripts).
