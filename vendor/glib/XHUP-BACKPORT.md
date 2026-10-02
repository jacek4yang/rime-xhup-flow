# Narrow upstream soundness backport

Upstream crate: glib 0.18.5, MIT, unmodified license retained.
Crates.io archive SHA-256:
`233daaf6e83ae6a12a52055f568f9d7cf4671dabb78ff9560ab6da230ce00ee5`.

Advisory: RUSTSEC-2024-0429. `VariantStrIter::impl_get` passed the address of an
immutable binding to a C output parameter; the C function writes through it.

Backport exactly the two-line fix present in gtk-rs-core tag 0.20.0:
`glib/src/variant_iter.rs`: declare `p` mutable and pass `&mut p`.
Source: https://github.com/gtk-rs/gtk-rs-core/blob/0.20.0/glib/src/variant_iter.rs

No public API/version change; GTK0.18's compatible dependency remains intact.
Cargo's package audit can still report the vulnerable original version number.
Any exception must verify this local patch/source, not blindly ignore the ID.
Remove this vendored patch when the Tauri/GTK dependency line accepts fixed glib.

Only registry bookkeeping `.cargo-ok`/`.cargo-checksum.json` is omitted. No
other upstream implementation was changed. Project regression tests exercise
forward/backward/nth/mixed string-array iteration with UTF-8 values.
