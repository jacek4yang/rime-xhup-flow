# Dependency security review — 2026-10-02 (F16, ongoing)

An audit exit code is not a claim of no risk. Initial `cargo audit 0.22.2`
(RustSec revision `6de4455`, database updated 2026-10-01) reported zero known
vulnerability entries but seven warnings: glib 0.18.5 unsoundness plus six
unmaintained-package warnings. These require disposition, not suppression.

Initial workspace pnpm audit: 51 advisories (2 low, 26 moderate, 20 high,
3 critical). Targeted compatible upgrades reduced this to 36 (19 moderate,
14 high, 3 critical). No remaining reported path begins at Trainer; remaining
paths originate at the miniapp/Taro toolchain or its optional H5 components.
This is a point-in-time finding, not a claim that all paths are unreachable.

## Applied upgrades

Webpack 5.91.0 → 5.111.1; same-major patches for brace-expansion (1/2/5), PostCSS8,
qs6, fast-uri3 and http-cache-semantics4. Taro itself remains 4.2.1: testing 4.3.0
did not remove the remaining vulnerable chains, so unrelated churn was reverted.
The old webpackbar progress UI supplied obsolete Webpack options; only that
presentation plugin was removed. Compiler validation, optimization and error
checks remain enabled. Both production builds, miniapp55 tests, Trainer111 tests
and both typechecks passed. The existing unused Taro Vite4 peer expectation
still conflicts with installed Vite7; miniapp uses Webpack, Trainer uses Vite7.

## Remaining review, NOT blanket accepted exceptions

- Taro CLI template/download/archive tools: got8/9, git-clone0.1, decompress4,
  adm-zip0.5, http-cache-semantics3, decode-uri-component0.2 and glob10.
  Current release scripts compile committed source, not remote templates, but
  explicit per-advisory bounded exceptions or compatible fixes are still needed.
- Optional H5 tooling: webpack-dev-server4, dev-middleware5, uuid8, node-forge1,
  esbuild0.21. Normal Trainer packaging does not use these chains. Do not expose
  these development servers to untrusted networks or interpret that restriction
  as a fix to the dependencies.
- Webpack CSS/HTML/minification: PostCSS7, html-minifier4, serialize-javascript6.
  Build-time trusted inputs reduce exposure, but advisories still need a durable
  per-ID/version/expiry disposition.
- Swiper11 has a critical reported advisory. Current miniapp is WeChat-only and
  contains no Swiper import; optional H5 reachability must be checked explicitly.
- Rust glib `VariantStrIter` output-pointer unsoundness (RUSTSEC-2024-0429)
  is mitigated by merged PR #172's exact two-line upstream backport to vendored
  glib0.18.5. `tests/security/check_glib_backport.py` checks 121 upstream file
  identities, the exact patched source hash, Cargo binding and bundled notices.
  Debug and optimized string-array iterator regressions passed. See
  [backport provenance](../vendor/glib/XHUP-BACKPORT.md). This is source-specific
  mitigation, not an advisory-wide ignore: Cargo audit may still flag the unchanged
  version. Unmaintained Rust dependencies and all remaining frontend advisories
  above still require disposition; F16 is not closed.

No stable-release security clearance is implied by these partial remediations.
