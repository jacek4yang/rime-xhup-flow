# Source reconstruction and historical decisions

Three different claims must stay separate:

1. **Rebuild from committed inputs/decisions**: pinned normative snapshots and
   Rust generation produce deterministic artifacts.
2. **Reconstruct from upstream files**: requires a retained extractor, pinned
   upstream revision, exact input hashes and byte-equality comparison.
3. **Repeat a model's historical decision**: a temperature-zero API request does
   not pin remote weights, infrastructure or behavior and is not deterministic
   provenance.

Core linguistic readings now satisfy (2): the standard-library extractor consumes
the exact MIT pinyin-data inputs, normalizes tones/ü/ê, takes the specified union,
and independently reproduces all 8,580 rows without reading those rows as an
oracle. The input manifest, tests, CI download/hash checks and output comparison
live under data/hanzi. Updating readings requires reviewing both source and
derived changes; changing expected counts alone cannot bless different bytes.

Wanxiang words already have retained offline Rust extractors and pinned Git blob
identities (data/words/README.md). Official/attested encoding facts remain
separate from linguistic readings; this reconstruction does not invent pinyin
for attested input codes or claim an official dictionary redistribution grant.

Historical Sogou category/classifier snapshots are **research-only**, excluded
from production dictionaries, compiled CLI payloads and distribution archives
by clean-v1 gates. Their earlier 55,684 remote-model decisions are not reproducible
model inference and are not current production dependencies. Preserved decisions,
prompts and source metadata can explain an old snapshot, but cannot establish
stable model weights or a redistribution license. A localhost API endpoint
never establishes local inference. No rerun or legal authorization is fabricated.

Corpus-derived regression fixtures likewise do not become independent test sets
because a file is called “test”: all KDConv upstream splits were pooled, and the
top-frequency replay sample overlaps those statistics. The committed replay
fixture contains public source sentences, so “only aggregate counts / no raw
sentences in the repository” is false. This is separate from runtime privacy:
the shipped input method never uploads or persistently logs user keystrokes.
