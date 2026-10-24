# Engineering handoff

Living handoff record for the v2.1 consolidation. Update after each milestone.
Free of secrets and personal data by policy.

## Snapshot

- Date: 2026-10-24 (updated after M1 hardening + M2 delivery).
- Main: `92173c92c4e037eb63310eed355875416971279b`.
- Remote branches: `main` only (verified with `git ls-remote --heads origin`).

## M1 hardening + M2 delivery record (2026-10-24, PRs #242-#246)

Five disjoint PRs, each with failing repro first, all CI-green on their
branches, merged sequentially, then revalidated on the combined tree.

- `#242` (squash `af5e07d`) keystroke counting contract. The completion event
  derived `keystrokes` from code length (`activeCode().length + wrong`), so
  backspace-retyped keys, route-switch retypes and abandoned prefixes were
  invisible to per-question events while the session counter counted real
  letter-key events; daily stats and chapter per-question accuracy inherited
  the understated denominator. The engine's internal `applyQuestionProgress`
  call also omitted `keystrokes`, so in-session `progressById.accuracyEwma`
  was always null (host stores updated it - a three-way divergence).
  Fix: per-question counter incremented exactly where the session counter
  increments, reset per question; event and engine evidence share it.
  Invariant test: sum of per-question event keystrokes equals session
  keystrokes. Also unified the sub-1s daily KPM/CPM rule into shared
  `dailyKpm`/`dailyCpm` (desktop trend + miniapp today chips). Six-scenario
  x four-dimension stats semantics anchored as tests; B5 review contract
  untouched.
- `#243` (squash `70a4aa4`) chapter evidence sufficiency + derivation cost.
  Counterexample: 19 migrated items (null EWMA, high mastery) + 1 new-evidence
  item (EWMA 1.0) -> chapter marked mastered on one new sample. New rule:
  when any new evidence exists, `mastered` needs >= `minNewEvidenceItems`
  (default 5); zero-new-evidence keeps the legacy bestAccuracy fallback (V3
  users' states stable). Performance on the real dataset (28,851 entries,
  mastery chapter 117k related items, 4,259 practiced): 67.65ms -> 4.36ms per
  full derivation pass (15.5x) via per-(index,chapter) WeakMap memoization
  and inverted traversal over progressById; results byte-identical to the
  naive path (equivalence test).
- `#244` (squash `7fe2f68`) twelve-mode behavioral combos (engine level, real
  data): illegal keys rejected wholesale; exactly one completion event per
  question and one session-completed; mid-session finish settles time once
  with no extra events; hint flow (learn intent + on-error trigger ->
  revealAnswer) never counts as independent recall; pre-existing progress
  accumulates rather than resets with stable ids; production level1 real
  full-code alternate route.
- `#245` (squash `6f41937`) backup capacity invariant. Real-structure probe:
  ~4,800 progress entries (heavy user, months) already exceed the old 2 MiB
  import cap in pretty form, and export had no guard - "export succeeds,
  own import rejects". Fix: compact serialization (machine-consumed doc),
  cap raised to 16 MiB (measured: 43,577 entries ~ 16.7 MB, JSON.parse ~100
  ms), export-side guard throws BackupError over the cap. Also documented
  the three independent version systems (desktop STORAGE_VERSION=4 zustand
  persist, miniapp STORE_VERSION=3 local doc, shared BACKUP_VERSION=3
  cross-host exchange) without merging numbers; unknown-field forward
  compatibility anchored.
- `#246` (squash `92173c9`) miniapp learn-center honest coverage. The miniapp
  has no lessonEvidence bookkeeping, so chapter *state* is not derivable
  locally: no state badges, no fabricated history; real coverage counts
  (practiced/related) are derived from local progress via
  `chapterCoverageById` (evidence map deliberately empty; return type has no
  state field) and shown per chapter, refreshed on page re-show.

Post-merge combined-tree validation (local, 2026-10-24): core 242/242,
trainer 89/89, miniapp 76+1, three typechecks clean, both production builds
pass; cloud CI dispatched on main (run `40122221928`).

## M1 learning-path and review-level record (2026-10-24)

User report: "the level shown in the UI never changes". The review scheduling
contract itself was verified correct; the gaps were presentation and honesty of
derived state. Two PRs landed, plus a dependency wave.

- `#239` (squash `9d4dc13`): shared review-status explanation core
  (`trainer-core/src/learning/review-status.ts`): `explainReviewStatus`
  derives level / due / capped / next-due countdown / hold reason
  (unverified, due-now, not-due, hesitant, lapsed, capped) from persisted
  state only; `formatDueIn` keeps wording identical on both hosts. Desktop
  ReviewView and the miniapp mistakes page render the reason and countdown.
  A `useNow` low-frequency clock (minute tick while visible, immediate
  correction on focus/visibility) replaces one-shot `Date.now()` snapshots on
  both hosts. 9 core tests + 2 controlled-time component tests.
- `#240` (squash `0e4a4c7`): chapter mastery honesty.
  "mastered" now requires sample sufficiency (>= 20 distinct practiced
  items), per-item accuracy EWMA (new `accuracyEwma`, alpha 0.3, computed in
  the shared core from keystrokes) instead of the lifetime-sticky
  `bestAccuracy` (kept only as a legacy fallback for migrated records),
  recency from `lastPracticeAt` only (reopening a chapter page no longer
  extends mastery), and the UI shows coverage counts (practiced/related).
  Storage version V3 -> V4; old data gets `accuracyEwma: null`, never
  fabricated history. Chapter pools were measured (9k-117k items), so a
  coverage-percentage gate is not feasible; counts are displayed instead.
- Dependency wave on the same day (ride-along commit in `#239`,
  `d963843`): body-parser `GHSA-27r3-jxmh-p828`, undici
  `GHSA-4qv8-2p9j-2685` / `GHSA-f72v-6w45-q472` / `GHSA-xjj6-f33q-929c`,
  serialize-javascript `GHSA-m5mm-p8h7-m29v`, fast-uri (4 advisories);
  same-major upgrades plus overrides; `pnpm audit` returns zero advisories.
- CI evidence: runs `40079962205` and `40083501108` both fully green
  (Rust workspace, trainer frontend, librime smoke, native smoke matrices).

### Practice-penalty audit (task section 2.5) - recorded, NOT changed

The scheduling contract is untouched. Audited semantics with precise
counterexamples; any semantic change requires its own PR with migration and
regression tests, per the task guardrails.

1. A legal full-code completion of a shortcut item (route `alternate`)
   counts as `imperfect`: `wrong + 1`, mastery -15, review level reset to 0,
   even with zero wrong keystrokes. Deliberate for shortcut-recall practice
   (engine contract B5: the primary route is the trained skill), but it
   conflates typing accuracy with recall quality in the `correct`/`wrong`
   statistics shown to users.
2. Hint-then-correct counts as `wrong` despite zero wrong keystrokes (same
   conflation; defensible for spaced repetition, misleading as "accuracy").
3. Hesitation (over 8s/char) still counts as a correct completion; only the
   level hold differs - reasonable, keep.
4. An early repeat with an error resets the review level and sets a 10-minute
   retry: defensible (an error is forgetting evidence), keep.
5. Proposed (not implemented): split item statistics into typing accuracy
   (key-level, route-agnostic) and recall quality (route/hint/independent)
   with additive new counters, so history is never reinterpreted; stats UI
   would show both dimensions with explained semantics.

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

1. Trainer M2 residuals: real-host UI automation on desktop Tauri (current
   component tests run in jsdom); miniapp on-device WeChat DevTools
   acceptance stays UNVERIFIED; the "typing accuracy vs recall quality"
   split counters (documented in the penalty audit below) remain a proposed
   semantic change awaiting a dedicated decision PR.
2. Behavior acceptance on real frontends for `#150`/`#151`/`#219`/`#223`. The
   runtime-level `#223` contract is proven on librime 1.16.1 (joint
   regression); the GTK3/Fcitx cold-start issue `#219` remains open, and real
   Weasel/GTK platform runs are pending.
3. Independent quality evaluation (frozen `#229` dataset) and performance
   qualification (`#225`) before any decision to enable `joint_decoder` by
   default; the switch ships default-off. Also verify `xhup-cli learning
   import` against the upstream `-e`/`-r` restore defect (item 3 above).
4. Full regression gate on the current main, reproducible packaging and
   artifact hashes (milestone M5).
5. Platform acceptance matrix and the release-readiness decision. 2.1.0 GA
   stays blocked while `#219`/`#223` platform evidence and `#225` performance
   gates are incomplete.
6. Local branch hygiene: 74 local branches with `[gone]` upstreams are kept
   for now; several contain pre-squash-era commits whose content is in main
   but whose SHAs are not ancestors. Audit before deleting.

## Environment notes

- The gh token now carries the `workflow` scope (required to push workflow
  file changes; granted via device-code flow on 2026-10-24).
- The WSL 1.16.1 audit environment lives under `~/xhup-audit` (generated
  package copy, pinned lua plugin build, `tests/librime` scripts).
