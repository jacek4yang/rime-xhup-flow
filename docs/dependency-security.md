# Dependency security review — 2026-10-02 (F16)

An audit exit code is not a claim of no risk. Initial workspace pnpm audit
reported 51 advisories, intermediate compatible patches left 36, and the current
locked frontend dependency graph reports **zero advisories at every severity**
(including development tools). No advisory IDs are ignored. CI runs both the
complete audit and local integration tests for the two dependency adapters.
This is a point-in-time database result, not proof of absence of vulnerabilities.

## Frontend remediation

The exact versions and integrity bindings are in `pnpm-lock.yaml` and
`pnpm-workspace.yaml`. Besides same-major patches, the remediation upgrades
Webpack/dev-server/middleware, esbuild, glob, Swiper, serialization, adm-zip,
URI decoding, UUID, PostCSS and the package-update checker. Taro remains 4.2.1.
The obsolete webpackbar presentation plugin stays disabled; compiler errors,
optimization and validation are not disabled.

Two adapters under `patches/` are applied by pnpm with lockfile-bound patch hashes:

- `download-git-repo`: replace `git-clone` with option-delimited `execFile` Git
  calls, refuse option-like/control-character checkout refs and custom executables,
  preserve legitimate checkout and `.git` removal; dynamically load maintained
  `@xhmikosr/downloader`, mapping archive options into `decompress` and HTTP options
  into `got`. The vulnerable legacy archive/downloader chain is no longer installed.
  Numeric legacy timeouts are mapped to request timeouts. Callers needing other
  HTTP controls use `got`; extraction controls use `decompress`.
- Taro Webpack runner: use `html-minifier-terser` instead of abandoned
  `html-minifier`, and await async compression in both normal and independent
  package build hooks. Compilation tasks are held in a WeakMap, isolated between
  compilations. Errors reject the build; incomplete assets are not accepted.

No minimum-release-age exclusions were added. An initially investigated newer
minifier was rejected rather than relaxing that protection. Production miniapp
builds enable XML minification so the patched path is exercised, not dormant.

Validation: both production builds, workspace typechecks and frontend tests
(Trainer 111, miniapp 55, plus shared core) passed. Four local security integration
tests cover legitimate Git checkout and injection rejection, actual local HTTP
archive extraction and traversal rejection, concurrent independent-package
compression, and minifier error propagation. Test archives contain only public
synthetic data and are deterministic. See `tests/security/frontend-dependencies.test.cjs`.

## Rust — separate, not audit-cleared by the frontend result

Initial `cargo audit 0.22.2` (RustSec database revision `6de4455`) reported zero
known vulnerability entries but seven warnings: GLib unsoundness plus six
unmaintained-package warnings. The GLib output-pointer bug (RUSTSEC-2024-0429)
is mitigated by merged PR #172's exact upstream backport to vendored 0.18.5.
`tests/security/check_glib_backport.py` checks 121 upstream file identities,
exact patched source, Cargo binding and bundled notices. Debug and optimized
iterator regressions passed; [provenance](../vendor/glib/XHUP-BACKPORT.md) records
the scope. Version-based audit may still flag that unchanged version; no global
ignore is warranted.

A refreshed Rust audit on 2026-10-02 reports zero vulnerabilities and six
unmaintained-package warnings: `proc-macro-error` 1.0.4 in GTK macros, plus
`unic-char-property`, `unic-char-range`, `unic-common`, `unic-ucd-ident` and
`unic-ucd-version` 0.9.0 under Tauri's URLPattern parser. The local source-patched
GLib is not listed as a registry warning; its source gate remains mandatory.
The [source-bound maintenance migrations](rust-maintenance-patches.md) now remove
all six warnings rather than ignoring them: maintained macro diagnostics and
Unicode identifier predicates replace the retired dependency chains. The refreshed
Rust audit on this revision reports **zero vulnerabilities and zero warnings**.
CI denies warnings and checks exact vendor provenance; no blanket audit exemption
is used. This does not retroactively qualify the earlier unpatched revision. This frontend remediation does not itself close F16, native
decoder qualification, or stable-release acceptance. Windows/macOS manual testing
is delegated to users as described in README; untested results remain unknown.
