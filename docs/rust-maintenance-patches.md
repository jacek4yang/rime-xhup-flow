# Rust dependency maintenance migrations

These narrow source-bound migrations remove the six unmaintained dependencies
reported by the GTK/Tauri chain, rather than suppressing their advisory IDs.
The current Cargo audit reports zero vulnerabilities **and zero warnings**.
This is a point-in-time database result, not a proof of perfect security.

## Exact scope

- `glib-macros` 0.18.5 and `gtk3-macros` 0.18.2: retain published source and APIs;
  replace `proc-macro-error` 1.0 with maintained `proc-macro-error2` 2.0.1. Add its
  canonical-name alias, which the new attribute expansions require.
- `urlpattern` 0.3.0: replace the five-crate UNIC identifier-table chain with
  `unicode-id-start` 1.4.0. Only two predicate calls change. This keeps ECMAScript
  ID_Start/ID_Continue semantics, **not** Rust's different XID predicates. Updated
  Unicode data intentionally accepts newly standardized scripts; `$`, `_`, ZWNJ
  and ZWJ handling remains in the upstream tokenizer. Tauri's URLPattern API and
  regex implementation are unchanged.
- `proc-macro-error2` 2.0.1: one `pub extern crate proc_macro` visibility correction
  prevents the compiler's `pub_use_of_private_extern_crate` future error. Diagnostic
  behavior and messages are not weakened or bypassed.

The crates keep their upstream versions and are bound explicitly through Cargo
path patches. The lockfile removes the retired packages altogether; no audit
ignore or local facade re-exporting vulnerable implementations is used.

## Provenance and notices

Sources were extracted from Cargo's published `.crate` archives. Each vendor
folder records archive SHA-256 and every original file SHA-256 in
`XHUP-UPSTREAM.json`. `tests/security/check_rust_maintenance.py` reverses only the
listed exact patches and verifies 103 original source identities, pinned inventory
and notice bytes, complete file sets, no symlinks, and Cargo/lockfile bindings.
It also rejects reintroduction of the retired dependency chain. Mutation tests
verify that source additions, edits, missing bindings/notices and retired packages
fail the gate.

The macro crates omit root notices from their published archives. The bundled
MIT LICENSE/COPYRIGHT are the gtk-rs project notices already preserved in
`vendor/glib`; failed fetches of old upstream refs were not presented as successful
provenance checks. URLPattern's missing root MIT notice was retrieved at its
published VCS commit `ea97a5de9740a7eda311281828b3270d92fb1afb`. The maintained
macro-error crate's published license files are retained unchanged.

Trainer's Tauri resource map carries all seven maintenance-crate notices alongside
both GLib notices in `licenses/`, including compile-time macro notices rather than
assuming macro expansion eliminates attribution obligations. The integrity gate
rejects omitted or colliding resource mappings. These checks bind packaging inputs;
final installer extraction must still verify actual notice bytes. They are not a
complete third-party license/SBOM review of all transitive dependencies.

## Validation

- Trainer check passed without the prior future-incompatibility warning.
- Focused integration checks exercise GLib derive output, Unicode parameter names,
  new-script identifiers and invalid identifier starts.
- The four upstream URLPattern test groups, including its fixture suite, passed
  using an exact copy of the patched source outside the workspace (so upstream
  test-only dependencies resolve independently).
- CI checks source identities and mutations; the normal workspace/Trainer builds
  and tests compile the migrated macros and exercise the consuming application.

The separate GLib iterator correction and its 121-file provenance gate remain
mandatory. These migrations do not qualify native input behavior or publish a
stable release. Windows/macOS manual acceptance remains the README user checklist.
