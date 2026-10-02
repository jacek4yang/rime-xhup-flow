# Native-tail policy — NOT qualified for release

The proposed `native_tail` translator delegates lexical search and sentence
scoring to native `table_translator@flow`. It issues at most six queries, inspects
at most two candidates each, and only handles ASCII input of 3–128 keys. Three
queries cover pending suffixes and three insert a native space delimiter at a
possible final 2/3/4-key boundary. Genuine native candidates are retained, with
outer end positions mapped back to the unmodified raw input. The `full_span`
filter swaps same-text/start objects only within its first 32 candidates so a
partial phrase does not hide the complete candidate before uniquification.
These bounds do not prove native query time or all-path coverage.

## Evidence and counterexample

A generated clean-v1 package passed the earlier 89-check suite with learning off,
learning on, and a new process. Exported native records included `你@ni` and
`好@hcnz`; selecting a pending prefix preserved its final `d`, and adding `e`
committed the intended sentence exactly once. This demonstrates those cases,
not general learning retention/size bounds or target-platform acceptance.

Adding six independently authored inputs produced **111 checks, 2 failures**:

- `nihcnzqu` lacks selectable `你好去` within the top 256.
- `nihcnzjbzqu` lacks selectable `你好进去` within the top 256.

The menus instead expose `你好耨去` / `你好耨进去`. A four-key code at an
internal boundary can still be lost by the native single-best path. Fixing only
the last boundary does not address that case. The failed assertions remain hard
failures in the real replay gate. Do not merge this candidate implementation or
claim F01 is resolved based on the narrower matrix.

The 555 recorded keys from the earlier three runs had local key-to-first-menu
p50/p95/p99/max of 7.537/8.876/9.711/10.967 ms. This is one Linux host, not a
performance acceptance bound; it predates the independent fixtures. The separate
bounded probe includes scanning up to 256 candidates and is not menu latency.

An isolated internal-boundary ablation (query each single cut, without changing
corpus weights) passed those six probes: 119 checks, zero failures. Adding two
repeated ambiguous-boundary probes then failed 2 of 121 checks:

- `nihcnznihcnzqu` → `你好你好去` absent from the top 256.
- `nihcnznihcnznihcnzqu` → `你好你好你好去` absent from the top 256.

Thus even enumeration of all **single** boundaries is not sufficient. This
ablation remains outside the repository's generated release package. Its 242
keys measured local menu p50/p95/p99/max 7.356/9.756/10.422/11.004 ms; that small
fixture does not establish worst-case work or acceptable query amplification.

Next: establish how the native decoder can retain competing internal paths,
rather than adding more special-case tail queries. Enumerating more boundaries is
not itself an acceptable architecture: arbitrary combinations, query cost,
preedit coordinate mapping, genuine learning identity, long-input behavior and
static compatibility all still need qualification. No stable release authorized.
