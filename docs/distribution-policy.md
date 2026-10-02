# clean-v1 distributable source policy (F03)

There is no documented redistribution grant for the Sogou-derived layer. Owner
risk acceptance, third-party mirroring and classifier decisions are not a grant.
The grant itself remains **EXTERNALLY BLOCKED**; v2.0.0 must not depend on it.

The normal generator now uses the eligible Wanxiang hot/extended layers and the
existing character/shortcut sources. Sogou shards and derived tags, verdicts and
protect-list are excluded from production dictionaries AND from embedded CLI
analysis data. Research data stays in the existing checkout/history, but is not
newly copied into packages. Source archives use `export-ignore` for its payloads;
only the research README and source manifest metadata remain. This does not erase
historical Git objects or claim they have acquired redistribution permission.

`data/xhup/sources.tsv` classifies Sogou as `research-only` with
`redistribution-not-authorized`, records the actual manifest hash and rejects its
use as production knowledge evidence. Generated `xhup_flow.sources.tsv` is an
owned package artifact containing clean-v1 policy and the source registry. Package
hashes and the sealed RC manifest therefore bind these policy bytes. Registry
metadata is not a copy of the excluded corpus.

The CI/native/packaging build checks the normal CLI compiler dependency manifest:
restricted payload paths must not appear. A changed restricted source manifest
must match its recorded SHA-256. Unit tests exhaustively require every produced
extended word/code pair to come from the eligible extended source and preserve
ALL eligible pairs; tests also verify restricted-only classifier-kept entries are
excluded. `research-sogou` explicitly enables local research tests only (even an
all-features non-test build cannot embed those payloads). Default tests can build
without the restricted payloads.

## Initial measured impact, not final quality qualification

Linux x86_64, Intel i9-12900H, debug generator, one warm run under concurrent test
load. Comparison uses preserved forensic package at audit baseline versus current
clean generation; version strings are still historical rc.2 and **these outputs
are not a new release candidate**.

| Observation | Audit package | clean-v1 local package |
|---|---:|---:|
| Flow dictionary total lines (including header) | 2,782,212 | 1,430,293 |
| Directory bytes (`du -sb`, includes directory metadata) | 64,753,150 | 37,091,958 |

The clean generation measured 34.75 s wall time, 610,436 KiB peak RSS. This is not
comparable final performance qualification, and the reduced dictionary must not
be presented as a performance improvement without acknowledging removed input.
The removed 1,351,919 dictionary rows include alternative projections, not that
many independently measured words. Character/open-composition paths remain, but
held-out top-k/latency/typing-cost impact and #150/#151 native replay still require
qualification. Do not replace this gap with vocabulary growth or invented scores.

All public final artifacts must be rebuilt through normal packaging with this
policy; old RC.2 artifacts are NOT made clean merely by editing repository policy.

## Archive and data notices

Packaging previously copied only YAML and Lua and silently omitted the generated
source-policy TSV. `tests/release/rime_archive.py` now requires the complete
22-file generated inventory, exact runtime/source-registry bytes, and eight
installation/attribution/license files. It creates a deterministic ZIP, reopens
it and compares every member byte-for-byte; missing, extra, private, duplicate,
changed or symlinked inputs fail. The same data notice/license texts are bound
into Trainer resources (separate from the vendored dependency notices).

The pinyin-data MIT notice is copied from the same pinned source revision
`923b108dc5d45dee061324c011b478fb649f8b73`, upstream `LICENSE` SHA-256
`9c048697be2502a16e8bcb282d5d465a07295b2def0ffb05a269c5d39dbe1586`.
The GPL v3 text incorporated by LGPL v3 is also supplied, copied verbatim from
the distribution's `/usr/share/common-licenses/GPL-3` (GNU license text),
SHA-256 `3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986`.
CC BY and corpus Apache texts retain their existing repository source notices.

The historical full `data/xhup/flypy_official_char_codes.tsv` also has no
documented redistribution grant. It is not a normal compiler input, is now
export-ignored from new source archives, and is explicitly forbidden in normal
CLI dependency evidence. The small fact-only oracle remains distinct.
Neither exclusion erases Git history or legalizes previous distributions.
This is a concrete package/source-notice correction, not full transitive
dependency-license/legal clearance or an updated runtime-quality result.
