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
