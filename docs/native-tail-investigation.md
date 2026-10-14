# Bounded native boundary planning

## Default fallback policy

The Flow schema has one native translation/learning provider, created by
`lua_translator@*xhup_flow.native_tail`. Static primary remains separate and
learning-disabled. `flow_lookup` is a **read-only** native dictionary binding;
it has no user database or memorization callback. The one native provider and
read-only Phrase factory use the encoder-capable `xhup_flow_user` dictionary.
The persisted `xhup_flow_user.userdb` name and spelled keys remain unchanged.
The unused `learn` configuration is removed; learning customization belongs
under `flow`, never under either read-only lookup namespace.

The planner obtains exact native lexical edges and minimizes their count with a
scalar dynamic program and deterministic tie order. **This is structural policy,
not native frequency scoring, a language model, or a claim of optimal linguistic
segmentation.** It creates no dictionary entries or Lua text candidate objects.
It asks native Rime to translate the resulting space-delimited code path and
retains the returned genuine native Phrase/Sentence. The original native first
five candidates (default visible page) keep their identity and order; at most eight
structural alternatives follow that prefix. Native quality values are unchanged,
and all remaining native candidates retain relative order. A short native stream
is emitted completely before alternatives. The same-text full-span filter may
substitute a longer interpretation of the identical text, not a different word.

The earlier +0.5/full-span promotion policy harmed an external wiki set: top1 fell
from 41.13% to 33.94%, despite improved pending-prefix and bounded reachability.
That negative result is retained by the independent-corpus study (#187). Removing
invented score preference is a general ordering invariant, not a word-specific
patch. The first set is now seen regression data; correction needs new evaluation.

Bounds per invocation:

- ASCII input 3–128 keys; outside this envelope use unmodified native translation.
- Native exact edges of 2–32 keys, at most one returned entry per lookup.
  The generated canonical prefix/length index currently limits each reachable
  start to at most five lookups; unsupported/custom profiles retain the
  `128 * 31` exhaustive bound. No corpus scan or corpus copy occurs in Lua.
- At most one full native query plus four boundary/pending-prefix queries,
  at most two candidates inspected per extra query.
- Preserve five original native candidates, append at most eight proposals, then
  stream the unchanged native tail; no score sorting or base-head materialization.
- The separate `full_span` filter swaps same-text/start candidates only inside
  its first 32 objects, so a shorter same-text phrase does not hide the full path.
- No file I/O, external model, network, persistent input log or Lua learning store.

Standalone 2/3/4-key codes do not receive a structural full-path replacement.
Missing lookup APIs preserve native translation; missing native translation
leaves the independent Static primary available. Errors remain explicit in the
module status, not evidence of successful full Flow operation.

## Opt-in bounded joint key/text search

`joint_decoder` remains **off by default** until independent quality and complete
compatibility qualification. Enabling it selects the implementation in
`joint_decoder.lua`, not the former inert compatibility module. The fallback
above remains available for unsupported bindings or customized dictionaries.

The search retains eight deterministic backpointer paths at each raw-key end,
rather than one minimum-edge predecessor. It accepts ASCII input up to 128 keys,
edges up to 32 keys, at most 256 offered edges per start, and the complete paths
plus each of the last seven completed frontiers. A completed prefix is not a
commit boundary. Each described path records raw start/end, remaining keys,
key/word segments, sources, lexical/context/user scores, deterministic tie order,
and its genuine native Candidate. Display separators never change raw spans.

`flow_aliases` reads the existing compiled Static dictionary without a user
database. Accepted one-level, PRIMARY and FIXED_FIRST spellings therefore become
search edges without adding aliases to the learning syllabary. Canonical matches
retain their original native IDs. Alias text is normalized through the generated
canonical reverse dictionary; a word absent as a single canonical entry can use
its canonical characters. Exact native rows confirm every edge. Per-query caches
hold lexical data only, disappear after translation, and never retain learned
results across append, backspace, cursor edits, selection or schema changes.
Normalization has an additional per-start lookup budget; budget misses are not
cached as dictionary absence.

The compiler emits `data/joint_scores.lua` from canonical character frequencies,
the actual generated Flow word frequencies, and the existing Apache-2.0 KDConv
bigram source. It reuses the Rust decoder's deterministic Q10 frequency reward.
The 16,384 highest-count context pairs use a capped reward; this is a small
frequency/context model, **not a calibrated language model or a quality claim**.
No research-only data or online inference is used.
The schema also binds the generated model identity. A stale `require` cache or
missing binding cannot silently score using a different deployment's data.

For completed retained paths, personalization reads the existing native writer's
dictionary using the canonical spelled key and exact candidate text. It uses
librime's own decayed weight only for an actual committed, byte-exact record.
It does not introduce a second writer, a commit counter or a persistent cache.
The native key layout and predictive lookup were inspected at librime commits
`295cb2ab68f89ee9d3237c7d4b8033bda3f3b635` and
`7bc3fb0005a03aff1c086611e4d52bb7f6444fdd`. An exact stored-key hit is limited to
one result; native traversal of deleted records or longer text prefixes is not
a constant-time guarantee. A personalization exception discards all user rewards
for that query while retaining lexical search.

Every emitted path constructs a real `DictEntry`/`Phrase` owned by the read-only
Memory whose dictionary **and Language** match the sole writer. The Code value
comes from `DictEntry.code`; the `Code()` shared-pointer wrapper is not a valid
replacement on the supported binding. No ShadowCandidate is added. Complete
joint paths precede ordinary dynamic candidates, except that an explicitly
learned, exact, full-span native first word retains first place. User-created
codes need not occur in the immutable beam lexicon. This protection keeps the
original native object, not a reconstructed text candidate. Pending paths follow
the native first page. The primary Static translator and its quality remain
unchanged; full Static compatibility must still be checked before default-on.

`tests/librime/run_joint_regression.py` runs in separate copied deployments,
never in the clean Static audit's database. Actual librime selection verifies
seven mixed-code sequences, middle-cursor edits, pending-tail continuation,
no dropped/duplicate commit, default quick hints, and no database creation with
learning off. Selecting the initially non-first `就是拟好` under `jqunihc`
promotes it on the next query and after process restart. Exports contain exactly
one correctly encoded `jqui nihc` record, with commits/tick 1 then 2. Quota zero
preserves typing, rejects the write, and leaves ranking and export unlearned.
Restoring a public synthetic 16-record old snapshot preserves its codes/counts
and tick 112 before new learning advances it to 113 and 114.

Native sentence selection of `你好你好去` now creates `nhnhq`; both decoder
policies recall it first after restart and commit it completely. CLI backup and
restore preserve all native keys, counts and quota. Native `UserDbMerger`
normalizes `d/t` to the snapshot clock, so the test compares effective decayed
probability within native six-significant-digit serialization precision, not
identical timestamp representations. These focused Linux checks passed; they
do not replace independent ranking, full Static traversal, paired performance
or GUI platform acceptance.


## Generated native lookup projection

The compiler projects the actual serialized Flow and learn dictionary rows into
`xhup_flow.data.native_edges`: first two keys → sorted possible byte lengths.
The current canonical tables contain only 2/3/4/6/8-key spellings, 414 prefixes
and four distinct length sets. A four-key projection would need 97,895 records;
the smaller projection avoids that loading/memory cost. It permits false positives,
but every remaining edge is still confirmed by the original exact native lookup.
It does not change predecessor selection, candidate ranking, native objects,
commit spans, or the single learning writer; it is not joint decoding.

The fixed supported Lua binding's `dictLookup` reads only `dict_`, not `user_dict_`.
Learned words therefore remain in the unmodified native provider; the projection
does not attempt to cache or filter that provider's results. There is no
per-input/global mutable lookup cache. Initialization clears the session's index
reference, and finalization releases it.

`flow_lookup/enable_compiled_index: true` explicitly opts the generated schema in.
Missing markers/APIs, another dictionary or prism, or any configured packs
retain exhaustive lookup. If intentionally replacing the canonical dictionary
under the same name, regenerate the package/index together or set this flag
to false in custom configuration. Never hand-edit generated files independently.
The schema also carries the projection's SHA-256 identity. A missing/mismatched
identity disables pruning, including when Lua's `require()` cache still holds
data from a previous deployment. The module identity is derived from sorted
prefix/length sets, not a hand-maintained version number.

Both source and compiled Lua suites check every eligible serialized dictionary
spelling against the projection, plus growing prefixes, backspace, raw offsets,
custom/legacy fallback and session reinitialization. The fixed-plugin Linux
focused replay, learning/restart and quota checks passed after this change.
A one-round instrumented 128-key smoke observed 17,801 lookups, maximum 283 per
key; instrumented timings are not latency evidence.

`tests/librime/compare_runtime.py` alternates baseline/candidate order, copies
synthetic deployments with empty user state on both sides, and retains all raw
traces and observable differences. `--require-equivalent` rejects any changed
candidate page, preedit, selection span or key handling. Uninstrumented runs
measure API latency; `--profile` is for allocation/GC/lookup counts. Neither is
GUI acceptance or an independent sentence-quality benchmark.

## Learning identity and editing

Do not wrap a planned native Sentence in another ShadowCandidate: the quick-hint
filter may add a shadow of its own. Native `GetGenuineCandidate` unwraps only one
shadow; the resulting inner Shadow is not a Phrase and silently loses learning.
The planner preserves the actual native object, changing only its outer raw
span and presentation, never its native quality. The regression gate commits a synthetic
sentence to an empty native database, exports its exact codes/counts, then repeats
in a fresh process. It requires one update per component per commit, not merely
that some older dictionary entry exists.

### Bounded native learning

There is one native writer, not a parallel Lua TSV store. Its memorization callback
checks the persisted native user-dictionary tick before delegating to native
`memorize`. The hard limit is 65,536 logical updates (a custom configuration may
lower it), with at most 64 committed elements and 64 total canonical/custom-code
units, 256 total text bytes, and 4,096 total serialized code bytes. An atomic
Phrase cannot bypass the aggregate bounds by containing many syllables in one
element. Native phrase encoding covers at most 20 characters. Preflight reserves
one update per direct element plus up to 32 native encoder DFS paths for a
multi-element commit; `max_homographs` does not bound those DFS paths.
Cross-commit history encoding is explicitly disabled: it can create zero-commit
records without advancing the positive native tick. Missing/enabled history
configuration fails closed before any memorization. Actual selections still
use the single native writer. An excessive imported tick pauses new learning.

The generator uses librime's `length_equal` rule field, not the previously
ineffective `length`. Initial-only native `stem` values are generated from the
same canonical codes and deduplicated by librime; they do not add syllables.
For the five-character regression above, rule correction alone produced 37
positive updates because multiple full-code variants encoded the same initials.
Generated stems reduced this to six (five components plus one encoded phrase),
with one learned encoded record. This is a measured update-count reduction,
not a typing-latency claim. Native encoder storage prefixes are decoded only
when producing replay keystrokes; backups preserve the original keys.

Quota/API/storage refusal keeps typing available and reports
`xhup_flow_learning_status`; the first Flow candidate carries a learning-paused
hint. Missing bounded-learning APIs fail closed, not into an unbounded writer.
A read-only provider is constructed first; callback assignment and disconnect
are probed on that object before creating a writer. Bindings without those APIs
retain read-only typing and report `bounded_api_unavailable`. Distro patches can
backport `Component.TableTranslator` without `UserDictionary.tick` (observed in
Ubuntu 24.04's September-2023-labeled package), so the constructor/date alone
is **not** a capability guarantee. The candidate writer receives a deny callback
synchronously, then its tick/configuration are validated before commits can run.
A missing tick disconnects it and retains the read-only provider. This may create
empty native metadata, but cannot learn entries; exports must remain empty and
byte-identical across restart. Missing storage is `storage_unavailable`, not
evidence that learning was disabled. A missing or overridden read-only flag
fails before constructing any provider.
CI separately exercises the actual distro fallback and full learning on
source-hash-pinned librime-lua commit
`68f9c364a2d25a04c7d4794981d7c796b05ab627`.
Read-only degradation is not full learning support.

This is a **logical update bound**, not a byte-exact LevelDB size cap or power-loss
durability guarantee. Export/reset/import are explicit ownership-checked CLI or
Trainer operations; reset affects native learning, not separate research state.
Native tests verify exact export counts across process restart and byte-identical
exports when a lower quota refuses a further update. The bound does not claim
that an arbitrary third-party custom translator cannot write to the same database.

Real-librime tests also cover 128-key input, caret movement inside composition,
backspace/reinsertion, selecting a prefix with multiple virtual delimiters,
finishing its pending tail, exact once-only commit, and process restart.

## Paired performance diagnostics

`tests/librime/compare_runtime.py` alternates baseline/candidate process order,
starts each side with empty synthetic user state, and retains all traces and
observable candidate/preedit/input differences. Both deployments remain intact.
Use an output directory and `TMPDIR` with sufficient quota; every trial copies a
deployment. A failed copy or compilation is a failed run, not a runtime result.
`--quick-hint on|off` is explicit in provenance and verified against each actual
session. The default is `on`, matching the generated Flow schema; context ranking
and session memory remain off. Static has no Lua quick-hint component.

The performance driver records both monotonic wall time to first-page
materialization and `CLOCK_THREAD_CPUTIME_ID` over that work. CPU time excludes
descheduling but includes the surrounding clock-read overhead; it is diagnostic,
not a replacement for user-visible latency, especially for microsecond Static
operations. Full regression replay does not enable these extra CPU clock reads.
The measurement provenance records inherited CPU affinity. On heterogeneous CPUs,
compare both versions on the same CPU class, for example with `taskset -c 0`,
and retain unpinned results separately rather than silently discarding outliers.
None of these native API measurements establishes GUI rendering latency.

### v2.1 prefix-index measurements with quick hints off

Runtime implementation `bff0e4e`, measurement harness `9e6a594`, compared with the
fixed v2.0.0 deployment on librime 1.16.1 / Lua 5.4.8. The i5-1240P has two CPU
classes: CPU0 is a 4.4 GHz maximum-frequency core thread; CPU8 is a 3.3 GHz
maximum-frequency core. Each row below uses five alternating process pairs,
10 repetitions, and 19,800 key events per side. No runtime parameters were tuned
between cohorts. Values are p50 / p95 / p99 in milliseconds:
These cohorts inherited the old harness's forced `quick_hint=off`; they do not
measure the default enabled hint filter. Both hint modes are now selectable,
and their results must not be pooled as if they were the same workload.

| CPU / learning | Key processing, old → indexed | Key to first page, old → indexed |
| --- | --- | --- |
| CPU0 / off | 3.539 / 5.197 / 5.478 → 3.359 / 3.929 / 4.100 | 3.547 / 5.205 / 5.488 → 3.367 / 3.938 / 4.110 |
| CPU0 / on | 3.697 / 5.668 / 5.945 → 3.527 / 4.298 / 4.469 | 3.705 / 5.677 / 5.954 → 3.535 / 4.308 / 4.478 |
| CPU8 / off | 9.490 / 11.777 / 12.196 → 9.174 / 10.158 / 10.374 | 9.504 / 11.793 / 12.211 → 9.189 / 10.173 / 10.387 |
| CPU8 / on | 9.553 / 12.104 / 12.605 → 9.247 / 10.548 / 10.882 | 9.567 / 12.119 / 12.620 → 9.261 / 10.563 / 10.896 |

Every individual Flow workload's p95/p99 processing, first-page wall time and
thread CPU time stayed within 1.10× baseline in these fixed-core cohorts.
First-session median/max and peak RSS also stayed within 1.10×. First-session
medians were 44.133→42.416, 57.102→58.142, 68.231→66.853 and 80.913→77.954 ms
in row order; this is a fresh-process measurement, **not a cold disk-cache test**.
Peak RSS was 34,916→34,944; 37,024→37,032; 34,868→34,952; and
37,016→37,060 KiB. These observations do not establish a startup speedup.

Static first-page p50/p95/p99 is reported in **microseconds**, not relative
near-zero ratios: CPU0 1.987/6.399/18.309→1.993/6.361/17.352;
CPU8 4.715/11.064/23.083→4.726/10.531/22.956. Each also has five process pairs.
All recorded candidate heads, raw input, preedit, selection ranges, page size
and key handling were identical in the six cohorts.

The earlier unpinned run is retained: three learning-off workloads exceeded the
1.10× p99 threshold in one candidate process. The old trace did not record CPU
affinity per key; its cause cannot be established retrospectively. Fixed-core
results control a known heterogeneous-CPU confound, not erase that failed cohort.
Two interrupted runs also remain: `/tmp` quota exhausted during copying, then
during compiler temporary output. Home-backed output and `TMPDIR` completed the
remaining runs without modifying user caches.

Separate instrumentation (11,880 keys) measured Lua-issued `Memory.dict_lookup`
calls 7,877,280→1,216,170, maximum per key 2,172→283. This counter does **not**
include dictionary searches inside TableTranslator. Native translator query
calls remained 42,600. Lua allocation calls were 14,602,387→6,738,481 and growth
bytes 1,062,259,384→533,378,235; incremental GC remained 19,890. No instrumented
latency is used as a performance result.

In one continuous 100-round session (39,600 keys), each round's Lua live bytes
remained exactly 8,467,594 before / 8,491,539 after; statm RSS remained exactly
39,288 / 39,284 KiB respectively. Compared with PR3's compiler output, compiled
Lua grew from 1,796,779 to 1,812,410
bytes, including the 14,578-byte index. There is no session lookup cache.

Complete-menu enumeration was measured separately, three pairs × three
repetitions: all 99 menu counts per side matched. `ni` still enumerates 21,468
candidates (median 2,726.584→2,807.198 ms); `jqu` has 806
(11.494→11.312 ms); the 128-key case has 34 (5.796→4.213 ms).
Full-menu text/order was not captured by this performance driver. The large
`ni` enumeration cost remains; it is not the normal first-page latency.

Raw evidence is retained under
`~/.local/share/xhup-flow/qualification/v2.1-native-index-bff0e4e/`, including
`analysis.json`, all process traces, and the initial-run archive SHA-256
`319e43c867a151a5a0a49fcbe1b268fed399bfe31306546b830226f0b53d8f08`.
These are focused native API observations, not full Static compatibility,
ranking-quality improvement, mixed-shortcut support, or GUI/platform acceptance.

### Default quick-hint-on measurements

The performance entry point now defaults to `--quick-hint on`, explicitly sets
the option, and verifies the actual session option in every process. Harness
`f82e9f73b13a240da357734313b61cb457c70bfb` measured a separate cohort; none of
the earlier hint-off traces were relabeled. CPU, alternating-order, fresh-user
state and plugin controls are the same as above.

Each row has five pairs × ten repetitions, 19,800 keys per side. Values are
keypress / first-page p50,p95,p99 in milliseconds:

| CPU / learning | Baseline keypress | Indexed keypress | Baseline first page | Indexed first page |
| --- | --- | --- | --- | --- |
| 0 / off | 3.523,5.167,5.452 | 3.357,3.912,4.062 | 3.531,5.175,5.461 | 3.365,3.921,4.072 |
| 0 / on | 3.715,5.661,5.984 | 3.534,4.325,4.532 | 3.724,5.671,5.993 | 3.542,4.334,4.542 |
| 8 / off | 9.456,11.724,12.164 | 9.067,10.158,10.403 | 9.470,11.740,12.178 | 9.081,10.173,10.418 |
| 8 / on | 9.611,12.150,12.640 | 9.431,10.706,11.012 | 9.626,12.165,12.654 | 9.445,10.722,11.026 |

All four cohorts had zero observable semantic differences. Every workload's
keypress, first-page and thread-CPU p95/p99 remained within 1.10× baseline.
First-session median/max and peak RSS also remained within that threshold;
first-session medians were respectively 41.956→41.768, 56.298→57.645,
68.395→66.865 and 80.464→79.014 ms. This does not establish faster startup or
measure learning-commit write latency.

Separate instrumented measurements (11,880 keys) recorded lookup calls
7,877,280→1,216,170, maximum 2,172→283; native queries remained 42,600.
Lua allocation calls were 14,655,579→6,789,319 and growth bytes
1,064,083,645→535,046,550. Incremental GC remained 19,890; generational GC was
zero. Over 100 continuous rounds, live Lua bytes stayed exactly
8,467,658→8,491,603 and statm RSS exactly 39,288→39,320 KiB, with no
round-to-round growth. Instrumented timings are not latency evidence.

The separate CPU0 full-menu cohort matched all 99 candidate counts per side.
Median enumeration-inclusive work was 2,888.399→2,878.386 ms for `ni`,
11.541→11.416 ms for `jqu`, and 5.746→4.218 ms for the 128-key case.
Full-menu text/order and GUI display latency remain unmeasured.
The evidence directory above contains `default-analysis.json`, its reproducible
`default-analysis-recipe.py`, and the independent `default-*` traces.

## Investigation history and corrections

The original final-boundary-only policy passed a narrow matrix but failed
`nihcnzqu` and `nihcnzjbzqu`. Enumerating every **single** internal cut passed those
cases, then failed repeated ambiguity in `nihcnznihcnzqu` and
`nihcnznihcnznihcnzqu`. All counterexamples remain hard tests.

An isolated C++ prototype passed with a larger path beam, and then with beam one.
However, examination of exact librime 1.16.1 source showed that its entry weights
were already logarithmic: the prototype's old-version conversion had reduced
them to constant edge costs. That result **did not demonstrate equivalence to
native scoring or establish a native graph-pruning bug**. Earlier cached older
librime source is not evidence for current behavior. The C++ plugin is not shipped;
its platform ABI/deployment complexity is unnecessary for the explicit structural
policy now tested through existing native Lua bindings.

Local isolated fixtures passed 129 core checks (including the independent
counterexamples), 19 editing/long-input checks, and exact single-writer exports.
Typical replay menu p50/p95/max was 5.186/8.635/16.034 ms; the 128-key stress fixture
was also measured separately. These are one-host measurements, not universal
platform guarantees or broad corpus-quality acceptance. The generated package has
also passed core replay with learning off/on/restart, extended editing, exact
single-writer counts, and quota refusal without mutation. Its separate native
learning audit verifies exact committed text, persistence, and actual CLI
export/reset/import. The local librime 1.16.1 full audit passed all 141,138
static codes both before and after learning, plus native learning management.
Other supported Lua/Rime versions and remaining release qualification must pass
before promotion. Windows/macOS manual acceptance follows
README. These historical results do not qualify a new release.

## v2.1 draft joint-search qualification

The opt-in draft remains disabled by default. On the frozen 7,388-case development
partition, source `435460321cb6b43d385a67b79ae7f6e77e379151` reduced Top-1 from
0.408365 to 0.391852, despite increased first-page coverage. This is a failed
quality gate, not release qualification; the confirmation partition remains
unobserved. These runs were not controlled paired performance measurements.

The original text-only corpus probe incorrectly counted 18 partial candidates as
complete matches. Actual native SDK inspection found spans `[0, raw_length - 1)`;
selection retained the last key and emitted no premature commit. Sending Space
then committed that remaining key too. The failures are retained, not relabelled
as correct complete sentences.

`candidate_match.cc` now obtains native candidate spans without wrapping or
copying candidate objects. Corpus ranking accepts only `[0, raw_length)` matches,
records text-only matches separately, and actually selects complete matches.
The summarizer rejects old traces lacking `complete_raw_span_v1`; historical
traces must be rerun, not patched with inferred span fields. A real 19-case replay
(one complete positive plus all 18 partial cases) passed 165 checks, retaining the
18 quality misses. Pending joint candidates display `待续:` and the unconsumed keys.

Native annotation/learning verification also selected a real `[USR]` candidate:
seed and two recalls advanced the native clock from 6 to 8; total compound
commits were 3 across the distinct encoded and ordinary-spelling records.
Export/import preserved those records, and another annotated selection advanced
the clock and total commits exactly once. Requiring one physical record per text
was an incorrect test assumption; native encoding namespaces remain untouched.

The draft's flat character scores also incorrectly let a common pronunciation
lend its frequency to a rare one: `di` preferred `的方` over `地方` in the
generated-model regression. Policy `joint-frequency-context-v2` groups scores by
canonical two-key sound and character, taking the maximum only across matching
shape variants. Alias edges use their resolved canonical sound. This is a scoring
input correction, not evidence that the broader Top-1 gate now passes.

The pre-correction development traces and SDK diagnosis are archived at
`~/.local/share/xhup-flow/qualification/v2.1-joint-4354603/native-joint-investigation.tar.zst`
(SHA-256 `6b8b2d157b0d68a1d620a46c792fab535b3e41c63a8f87456fab2aaa3b6f21af`).
The archive contains 4,432 files and is 5,989,657,972 bytes; tar comparison passed.
It is investigation evidence, not a qualified release package.

### Complete native first-choice contract (development)

The `0308710` reading-model correction was rerun against the same frozen 7,388
development cases, with the same `complete_raw_span_v1` harness for both sides.
Both runs had zero commit-contract failures. Top-1 was 40.8365% → 39.8078%,
Top-3 43.4759% → 53.7899%, first-page coverage 43.8820% → 58.9740%, and complete
selection/commit 44.7212% → 62.5609%. There were 193 lost baseline first choices,
117 gained first choices, and 41 lost baseline first-page cases. This remains a
negative default-enablement result; the confirmation partition is untouched.

The previous merge policy unconditionally put joint paths before native results
despite having no cross-model score calibration. The conservative replacement
preserves the first native candidate only when its raw span is complete and its
native entry has no remaining code. Joint candidates follow that object; when
the native head is partial or predictive, complete joint paths may precede it.
Static exact priority is unchanged. Joint path scores still rank competing joint
paths; they do not claim to be comparable to native translator quality.

The ordering regression failed before this change and passed afterward. Real
librime mixed typing, cursor edits, learning-off privacy, restart, quota and old
state restoration also passed. A preserved native Sentence naturally learns its
constituent entries, unlike the previously promoted atomic joint Phrase. The
old compound test incorrectly assumed that record layout independently of which
native object was selected. It now compares actual joint-on/off native learning
records, tick and persistent identity across selection and process restart.
This differential regression fails on the previous package and passes on the
new one; the separate joint-Phrase exact-once test remains. On librime 1.16.1 the
`nirxhcnz` control and new policy both incremented `你/nirx` and `好/hcnz` once per
selection, with ticks 2 then 4.

These focused results alone do not qualify the ranking policy. Development and
confirmation results follow; paired performance failed and platform acceptance remains pending.


## v2.1 首选保护冻结后的确认集（19f83fa）

在观察确认组排名前冻结运行时代码 `19f83fa`。全部 7,291 条事先划分的确认样本
使用相同 `complete_raw_span_v1` SDK harness 重跑 v2.0.0 与新方案；不删除失败样本。
两侧均退出 0，完整提交契约失败为 0。该语料已经完成首次观察，此后不能重新称为未见集。

| 指标 | v2.0.0 | 新方案 | 句子聚类配对 95% 差值区间 |
| --- | ---: | ---: | ---: |
| Top-1 | 39.9259% | 39.9259% | [0, 0] 个百分点 |
| Top-3 | 42.6005% | 53.6689% | [10.3357, 11.8125] 个百分点 |
| 可见第一页 | 43.0119% | 58.7299% | [14.8808, 16.6187] 个百分点 |
| Top-256 / 完整选择上屏 | 44.3012% | 61.8982% | [16.7343, 18.5527] 个百分点 |
| 待续目标前缀可达 | 36.4971% | 61.9942% | [24.5157, 26.5914] 个百分点 |

区间使用现有分析器的 1,999 个句子簇、2,000 次固定种子重采样。
旧首选丢失/新增均为 0，但有 **3 条旧第一页目标掉出第一页**，逐例保留。
排除已知 KDConv/PTT 重叠后的 7,171 条仅作分组报告，仍不能证明严格训练隔离。
开发组 7,388 条的 Top-1 同样保持 40.8365%，Top-3 为 54.6697%，
第一页 59.3530%，完整提交 62.5609%，旧第一页丢失 5 条。

这通过的是预先规定的聚合质量门槛，不是首选质量提升，更不是成熟拼音级首选能力证明。
Top-1 没有改善，所有不可达样本仍在分母中；全量 Static、性能修正后的重新资格验证、
当前产物平台验收与最终发布门禁仍须分别完成，默认开关继续关闭。

本机原始证据：`~/.cache/xhup-flow/v210-native-tmp/confirmation-native-first-19f83fa/`，
含两侧逐键 JSONL、部署文件哈希、输入/插件/二进制身份与 `analysis.json`。
分析器新增第一页聚类区间，并拒绝缺失/非法页长或与完整候选排名矛盾的第一页标志。
质量回放总耗时不是配对性能；性能驱动分别指定 `--baseline-joint off`
和 `--candidate-joint on`，记录实际开关及 ready/bounded 状态，禁止静默回退冒充启用测量。

## 四组联合搜索配对性能：未通过（harness 6eb754d）

冻结运行时不变，每组五个交替顺序进程对、每侧 19,800 次按键；
quick hint 开启，基线 v2.0.0 关闭 joint，新方案明确开启并核验 ready/bounded。
下表为原生 API 按键至第一页菜单，不是 GUI 显示延迟。

| CPU / 学习 | 基线 p50/p95/p99 ms | 新方案 p50/p95/p99 ms | 峰值 RSS KiB，基线→新 |
| --- | --- | --- | --- |
| CPU0 / 关 | 3.607 / 5.196 / 5.646 | 6.662 / 10.759 / 11.942 | 34,940→59,260 |
| CPU0 / 开 | 3.607 / 5.335 / 5.690 | 7.288 / 12.000 / 13.199 | 37,044→63,780 |
| CPU8 / 关 | 8.647 / 11.184 / 11.825 | 11.780 / 18.534 / 19.977 | 34,932→59,252 |
| CPU8 / 开 | 8.633 / 11.406 / 12.088 | 12.910 / 20.349 / 21.944 | 37,008→63,756 |

四组均未满足既定延迟/资源比值 ≤1.10 的门禁，不能默认启用。
进程初始化与首会话时间基本相当，但不包含 joint 首次按键时的延迟加载成本。
每组有 18,600 次菜单内容、3,550 次 preedit、5,100 次选择显示范围差异；
没有 raw/commit/handled/caret 差异，不能将菜单内容差异数量当成首选损失数量。
全部逐键记录、进程摘要及失败比值保留于
`~/.cache/xhup-flow/v210-native-tmp/joint-paired-6eb754d/`。

另以同一新包 joint 关/开进行插桩诊断（1,188 键，非延迟资格测量）：
Memory lookup 121,617→148,653，Lua 分配次数 684,999→13,987,041，
增长字节 53,838,725→1,118,066,180；回合末 Lua 分配器尚未释放的字节约 8.5→11.1MB。
只减少查询次数不足以修复此问题。关闭自动 GC 的额外函数级诊断显示，
词边提供器约 180MB、搜索其余部分约 69MB、路径展开约 91MB 的净增长；
诊断用 `rime.lua` 仅在隔离副本中注入，不进入发布包，也不用于声称生产延迟。
这为后续分配优化提供依据，并不改变当前性能门禁失败的结论。

## 查询内分配削减试验

只复用尚未展开的未来 beam 被淘汰节点；已经展开的祖先不复用。
同一次查询共享不可变词边，上限 1,024 项，canonical / static_alias /
spelled_alias 分开存储，避免归一化额度耗尽后的逐字兜底污染后来成功的整词编码。
不缓存候选或用户词典，不跨查询保留状态，不改变 beam、评分、排序或查询额度。

插桩配对每侧 1,188 键：分配次数 13,987,880→11,575,388，
增长字节 1,118,123,139→903,505,088；Memory lookup 同为 148,653，
native query 同为 1,185，语义差异为 0。峰值 RSS 59,168→59,136 KiB，
没有实质改善。证据目录为 `joint-edge-sharing-profile/`。

非插桩 CPU0、学习关闭、quick hint 开启，三个交替进程对，每侧 5,940 键：
原生 API 按键至第一页 p50/p95/p99 从 6.559/10.532/11.595 ms
变为 6.447/10.183/11.224 ms；语义差异为 0。
三进程最大 RSS 59,200→59,084 KiB。目录 `joint-edge-sharing-latency/`
保留逐键记录和双方生成物哈希。这只是旧 joint 与优化 joint 的单组试验，
不是相对 v2.0.0 的完整四组重新资格测量，不解除默认关闭的门禁。

纯 Lua 搜索回归 60 项通过，包括未来节点替换后祖先路径不变，以及
同词从逐字兜底升级为整词编码时身份不混淆。独立生成的实际包通过
35 个运行时来源身份校验、原生 joint 学习回归，以及回放中的 129 项 core 和 22 项 extended 检查。

另一次仅用于归因的构造器诊断中，模型首次加载约增加 2.8 MiB 匿名驻留，
ReverseDb 构造约增加 1.6 MiB 文件驻留，alias Memory 构造没有可见增量；
不能按 reverse 文件体积直接推断其构造器占用了全部 RSS 差额。
以上本机目录均位于 `~/.cache/xhup-flow/v210-native-tmp/`。


## 有界只读词表缓存试验

会话内 FIFO 只保留只读 Memory 查询得到的文本、权重与原生 syllable ID，
最多 512 个查询、2,048 行、262,144 字节文本/码串；过大的行不缓存。
不保存 Native Candidate、用户词典行、评分、归一化决策或 beam 状态。
schema/词典实例更换拒绝旧缓存，组件初始化、失败和销毁均释放引用。
追加、退格、编辑或用户学习不会改变只读词表；用户奖励仍逐次查询原生 userdb。
查询内归一化额度不因跨键命中缓存而增加，冷/热路径保留相同学习身份。

CPU0、学习关闭、quick hint 开启，三个交替进程对、每侧 5,940 键：
原生 API 按键至第一页 p50/p95/p99 从 6.536/10.318/11.403 ms
降至 4.668/8.714/9.784 ms；0 个语义差异，最大 RSS 59,172→58,024 KiB。
这是与前一 joint 实现相比，不是与 v2.0.0 相比的完整资格测量。
证据：`joint-readonly-cache-latency/`。

学习开启的插桩配对每侧 1,188 键，0 个语义差异：
Memory lookup 148,653→4,788（不含 UserDictionary 查询），native query 同为 1,185；
Lua 分配 12,344,675→8,566,883，增长字节 1,003,003,119→737,627,470，
增量 GC 同为 1,410，峰值 RSS 63,604→62,360 KiB。
证据：`joint-readonly-cache-profile/`；插桩时间不能作为生产延迟。

源码与编译产物的 Lua 套件均通过，搜索 74 项包含冷/热一致、编辑、
词典替换、负查找、密集同音桶、超大行和额度边界。
新的原生生成包通过 35 项身份验证以及混码、范围提交、学习关闭、
一次学习、重启、配额和旧记录回归；仍不是全量静态清单或 GUI 验收。
独立函数计时诊断中，383 次搜索约 0.649 秒，其中词边提供器约 0.220 秒；
22,542 次路径展开约 0.150 秒。首次评分模型加载约 13.334 ms。
这些是加包装器后的归因数据，不改变此前四组性能门禁失败及默认关闭的结论。

## 默认关闭路径的配对结果与真实 Linux 窗口

`d47684b` 默认关闭联合搜索，与固定 v2.0.0 源码比较；每组五个交替进程对、
每对十轮，每侧 19,800 键。四组均无候选/提交语义差异，所有 case 的逐键、
首屏和 CPU p95/p99 比率均不超过 1.10。按键至首屏汇总如下，单位 ms：

| CPU / 学习 | v2 p50 / p95 / p99 | 当前默认路径 p50 / p95 / p99 | 资源总门禁 |
|---|---|---|---|
| 0 / 关闭 | 3.549 / 5.181 / 5.588 | 3.377 / 3.986 / 4.329 | FAIL |
| 0 / 开启 | 3.751 / 5.669 / 6.050 | 3.549 / 4.351 / 4.640 | PASS |
| 8 / 关闭 | 8.832 / 11.248 / 11.834 | 8.382 / 9.983 / 10.305 | PASS |
| 8 / 开启 | 8.778 / 11.539 / 12.258 | 8.228 / 10.262 / 10.695 | FAIL |

CPU0/学习关闭的 initialize 最大值比率为 1.153464；
CPU8/学习开启的首次 session 最大值从 80.097357 增至 92.507926 ms，
比率 1.154944（中位数比率 1.0181）。保留全部原始失败，不修改 1.10 门限，
也没有证据将其直接归为环境噪声。不能把逐键结果写成整体性能资格 PASS。
证据目录：`default-paired-d47684b/`。

另用 Ubuntu 26.04 的 Fcitx5 5.1.19、fcitx5-rime 5.1.13、GTK3 和认证 Xvfb
运行真实窗口，发行版 librime 1.16.1 / Lua 插件
`1.16.1+dfsg1~git20250707-1build1`，不覆盖其动态库加载：
`q→去`、`jqu→就是`、`jbzqu→进去` 的实际 preedit、候选截图和完整提交通过。
`jbzqu` 的窗口显示 `jbz qu`，首候选为“进去”。
这只是三个真实前端场景，不是完整 Linux 平台验收，也不是冻结 RC 的收据。

早期驱动在冷启动立即发送 Escape/Ctrl+A/BackSpace 后未收到首题 preedit，
随后提交失败；所有失败日志保留。截图曾使用按键并被 X11 映射为 Alt+F12，
现在改为不注入按键的截图请求，并等待真实 GTK preedit。
冷启动控制突发场景保留为 `--cold-control-burst`，不能从失败推断产品根因。
永久入口使用私有 HOME、X11、D-Bus 和白名单环境，不加载宿主服务目录：

```sh
python3 tests/librime/run_gtk_fcitx.py \
  --package /absolute/generated-package --output /absolute/new-gui-evidence \
  --prefix /usr
```

它先通过现有编译器验证生产包，再部署可信快照；输出完整提交文本、preedit、
截图、日志和文件哈希。只使用合成输入，不读取用户词典。所有 GUI 结果均不能
替代 Windows、macOS、Android 验收或全部学习/升级/异常场景。

测试驱动可用 `--source-root /absolute/frozen-checkout` 指定独立生产源码，
仍须用该源码的编译器验证全部包身份；收据记录生产源码 SHA、脏状态和驱动哈希，
不把研究分支误当成 RC 来源。对冻结 `2.1.0-rc.1` 源码
`e739ab5dd2a0b833b95a051a126110215ece2918` 的 34 文件生产包，
上述三个真实输入、候选显示和完整提交均通过，`jbzqu` 截图首选“进去”。
证据目录为 `rc1-e739ab5/`；这仍不是完整平台验收。

永久驱动的发行版默认投递实测：基础三题通过，`--cold-control-burst` 失败，
目录分别为 `regression-d47684b/` 和 `regression-d47684b-cold-control/`。
在同一包、同一按键序列上，仅启用 `--sync-mode`，冷启动突发及三题全部通过，
证据为 `regression-d47684b-cold-sync/`；没有把该选项写入产品或用户环境。
固定 v2.0.0 的既有部署副本也在相同冷启动序列丢失首题 preedit，
证据为 `baseline-v2-cold-control-exclusive/`。副本仅把测试 schema 列表限定为
`xhup_flow`，避免本机未安装的八个其他方案导致部署失败；原始基线不修改。

核对的 fcitx5-gtk 5.1.6 源码为
`0828b59d90f92726f7302a3b4c9e42caeeb2a8a7`：
`gtk3/fcitximcontext.cpp` 的默认投递使用异步 D-Bus 回调，未处理的事件重新入
GDK 队列；reset 另行发送，`FCITX_ENABLE_SYNC_MODE` 是上游已有的同步选项。
[INFERENCE] 结果指向冷启动的异步投递/重置时序，但尚无完整调用轨迹证明根因。
这不是已修复的产品缺陷，不删除默认失败，也不以同步诊断替代默认平台验收。

### 冻结 RC 的六例首选与实际上屏

源码 `39e9cfc70139897429e00033d78823827da482d8`、同源 34 文件包，
默认 GTK/Fcitx 投递、隔离用户状态、逐键输入后只按空格选择首选：
`q`、`jqu`、`jbzqu` 和长串
`nihcvegeuurufawojtdesuduhduikeyi` 完整上屏通过；长串实际提交
“你好这个输入法我觉得速度还是可以”，没有残留组合。

`nihcnzqu` 的 preedit 为 `nihc nz qu`，实际提交“你好耨去”；
`nihcnznihcnzqu` 为 `nihc nz nihc nz qu`，实际提交“你好耨你好耨去”。
两例目标分别为“你好去”“你好你好去”，因此整套 **FAIL（4 PASS / 2 FAIL）**，
不是候选可达性通过。驱动保留每例截图、提交和剩余 preedit，继续收集后续
案例但最终非零退出，不删除失败样本。证据 `rc1-39e9cfc-six-complete/`。

正式发布阻塞 [#223](https://github.com/jacek4yang/rime-xhup-flow/issues/223)：
不能以基础三题或候选深处可达代替首选质量；不为这些输入增加特判。
尚未以这次观察证明是相对 v2.0 的新回归，且本研究联合搜索没有进入该 RC。

## RC.1 后修复的性能预算（用户授权调整）

本轮在新修复和测量前固定同机配对上限为 **1.20 倍**，替代后续资格使用的
1.10 倍预算。适用于每个场景的 p95/p99 按键处理、首屏与线程 CPU 时间，
进程初始化和首会话创建的中位数/最大值，以及峰值 RSS。固定 CPU0/CPU8、
学习开/关、默认提示开启、五对交替 AB/BA；不并发执行重型构建或审计。
持续相同工作负载的内存必须有界，提交、剩余按键、学习与 Static 合同不放宽。

这是明确的产品取舍，不是性能修复：此前按 1.10 判定的失败及原始轨迹原样保留，
新报告同时列原始比率与本轮 1.20 判定，不重写已经公开的 RC 证明。
不得因为新门槛仍失败而继续提高上限，不能用整体分位数掩盖单场景回退。
新联合搜索仍须独立实测；其历史约两倍延迟和约七成 RSS 增长不被本预算接受。
本调整不豁免质量、真实平台验收或任何安全/签名要求。

### 自动组句与精确词条的首选边界

RC.1 后诊断显示，联合搜索已经给 `nihcnzqu` 的 `ni hcnz qu` 路径最高分，
但 `complete_exact` 把原生自动 `Sentence` 与词典精确 `Phrase` 一概保护，
将“你好耨去”留在首位。重复输入同理。这是首选契约错误，不是目标词边缺失。

新 `flow/ranking_policy: joint-confidence-v1` 只允许自动 `sentence` 被联合路径
替换，要求完整消费、不同文本前两名分差至少 2048 Q10（两个 log2 单位），且
没有总词边截断。低置信、未知策略、精确词条和用户词继续保护原生首选；
联合第一名与原生文本相同时也保留原对象，避免将 Sentence 的学习改成 Phrase。
高置信时只把联合第一名放在原生整句之前，其余联合候选排在原生整句之后；
不再把整个 beam 放在原生首选前导致用户翻页。`taruhzbjji` → `踏入后半季`
的第一页可选与完整上屏已加入真实原生回归。
`native-prefix-v1` 恢复旧首选保护，`joint_decoder` 关闭仍回退原生路径。
该分差是保守排序规则，不宣称已校准的上下文概率。默认启用仍需独立质量、
Static、学习和本轮性能资格，不能凭几个首选例子通过发布。

### 联合路径分配与扩大开发集（2026-10-14）

针对 `5726d26` 的 7,388 条已见开发样本，双方使用
`complete_raw_span_v1`：Top-1 均为 40.8365%，Top-3 从 43.4759% 到
54.4667%，第一页从 43.8820% 到 59.2447%，完整选择提交从 44.7212%
到 62.5609%，提交合同失败均为 0。保留了全部损失：33 条旧首选下降、
33 条新首选提升、14 条旧第一页目标下降。它不是未见集证明，也没有证明
总体首选质量改善；不能用可达性增长替代 Top-1 门禁。

分阶段真实 Lua VM 分配计数发现，每次输出菜单时，为每条路径展开的
`sources`/`spans` 诊断数组会重复分配。运行时现保留原始 node 回溯及
keys/words/codes，只在诊断调用要求时展开这些数组；没有删去词边、缩小
beam、缓存用户数据或改变候选对象。诊断接口默认仍返回完整分段信息。

`compact-path-profile` 使用同一合成部署数据、联合开启、学习开启、
1 对交替运行、3 次回放、1,188 键，修改前后可见状态差异为 0：

| 指标 | 修改前 | 修改后 |
| --- | ---: | ---: |
| Lua 分配调用 | 8,566,145 | 5,964,837 |
| Lua 增长字节累计 | 737,577,671 | 566,747,142 |
| dict lookup | 4,788 | 4,788 |
| native query | 1,185 | 1,185 |

这些是插桩累计计数，不是峰值内存、时间或 GUI 延迟。修改后另一次 focused
原生回归通过混码、首选、编辑、完整提交及学习/重启/配额；不能据此宣告
1.20× 性能预算或全量兼容已通过。

`dc1c62c` 非插桩试测（CPU0、学习开、3 对交替、每次3轮）仍未达到预算：
v2.0 / 联合版本的总体菜单 p95 为 5.354 / 9.293 ms，p99 为
5.710 / 10.185 ms，峰值 RSS 最大为 36,992 / 62,260 KiB。
按案例的单字冷加载 p95 为 2.043 / 16.042 ms。数组优化有效不代表发布性能
已合格，必须继续处理未显示候选的物化及加载成本，不能提高已固定的1.20×门限。

### 菜单消费与待续路径物化

单独延迟待续候选没有收益：`full_span` 原先会预取32项，仍会触发所有待续
候选构造。现对已覆盖完整原始输入的前部候选直接输出：合法 raw span 不会
比原始输入更长，因此后面的同文候选不可能改善其范围。遇到首个短范围候选
后仍执行原来的同文交换；已输出项也计入同一个32项上界，不重开窗口。

联合搜索仍保留所有近期完整 frontier。完整消费与待续分组排序，待续组只在
菜单消费到原定位置时物化；各组内部用户读取失败撤销本组奖励，报告
`personalization_unavailable`，不撤销此前已成功读取的完整组结果。没有第二
写入器、没有跨查询用户缓存；源对象、原始范围和全部待续路径仍可选择。

`streaming-span-profile` 与 `accfc8c` 同数据、联合/学习均开、1188键比较：
可见语义差异0，分配调用5,963,038→5,413,030，增长字节累计
566,569,997→523,545,423。Lua 测试覆盖不可用尾流不阻塞完整第一页、全局
窗口边界、原生对象身份，以及待续用户读取故障；focused 原生提交/学习回归
通过。实际 GTK3 同步配置下六项首选/完整上屏亦通过。

这组优化仍未使性能达标。CPU0学习开3对3轮的 v2.0 / 当前版本菜单 p95
为5.492 /9.533ms，p99为5.923 /10.733ms，峰值 RSS 最大36,944 /62,296KiB。
证据保留在 `streaming-span-latency`；不能从分配减少推出尾延迟已改善。

搜索内部还会对共享末词边的相邻 beam 节点重复计算同一转移。现仅在当前词边
扩展期间复用最后一对相同边对象的纯函数评分，不缓存用户状态，也不改变
serial、排序、beam 大小或非法分值拒绝规则。已注定不能进入满 beam 的分数
先比较排名，再检查有限性，仍不允许 NaN/Infinity 进入路径。

`transition-reuse-latency`（CPU0、学习开、3对3轮）对比 `111da5d`：
菜单 p95 9.753→8.794ms，p99 10.830→9.860ms，线程 CPU p95
9.681→8.788ms，可见语义差异0；峰值 RSS 没有实质降低。
源/生成 Lua 全套行为测试、冻结68,842条提示映射和 focused 原生学习回归
均通过。该数据证明局部收益，不代表对 v2.0 的发布预算已满足。

后续按相邻相同编码复用范围校验，并在同一扩展中复用 beam 长度；没有改变
边数、保留路径或查询上限。`key-group-latency` 对比 `b302328`（同样
CPU0、学习开、3对3轮）：菜单 p95 8.472→8.284ms，p99
9.519→9.354ms，语义差异0；峰值 RSS 最大62,152 /62,300KiB。
Lua 搜索101项、native-tail 4,199项、full-span 1,507项断言及重新生成后
的 focused 原生提交/学习回归通过。这仍是局部试测，不是完整预算验收。

`joint-mappings-b302328` 的实际进程映射采样表明，新增驻留页不仅来自
Lua：联合模式的 `xhup_flow_user.table.bin`、`reverse.bin`、`prism.bin`
分别采到16,744 /10,992 /5,056KiB。单文件最大值不保证同时出现，析构时
采样也不能用于宣称其他词典没有加载；下一步须减少规范化反查触及的文件页，
而不是只根据 Lua 分配量推断内存改善。

### 构建期只读规范化索引

生成器从原有规范词典和静态层生成 `xhup_flow_alias_projection.dict.yaml`，
由 Rime 部署器编译成标准 ReverseDb；不增加用户词典、不改变原有 syllable-id，
也不引入新的持久化写入器。运行时只读该索引，不再为每个静态词文本扫描整个
规范反查表。索引保存原有前8个反查编码及前64项查询中的 ID、权重和未命中标记；
未命中仍消耗原有查询额度，缓存命中也不能扩大每次查询的32次预算。

生成规则按 librime `de4700e9f6b75b109910613df907965e3cbe0567` 的
syllabary 顺序、首重复保留和原生 float 权重实现。schema 绑定索引内容 SHA-256，
每条候选还须经原生 `Dictionary:decode` 验证编码身份。不可用 API、缺失索引或
身份不匹配明确禁用联合路径，保留普通原生输入。缓存仍限512项、2048行、
262144文本/编码字节，只缓存不可变词边，不缓存用户候选或查询决策。

`joint-alias-projection-package` 实际生成37项产物并通过源码→编译器→产物验证。
在 librime 1.16.1 和固定 librime-lua 插件上，原生门禁对全部108,208个静态
词/字、128,618条索引记录逐项比较实际反查顺序、ID、权重、8/64上限和 decode。
focused 输入、光标编辑、完整提交、学习重启、配额和旧记录回归通过。
`alias-projection-fallback-regression` 另在隔离目录删除索引或替换身份摘要：
均明确报告不可用，`ni→你` 仍完整提交且不创建学习数据库。

真实 `rime_deployer --build` 无手工补编译产出索引，Flow 78项、Static 73项
输入烟测通过；该次部署峰值 RSS 为1,050,028KiB。额外部署文件包括2,431,564
字节 reverse、4,644,716字节 table 和13,559,100字节 prism；运行时只读取
reverse，部署资源成本仍需与基线配对，不能只报告运行期节省。

此前原型的 CPU0、学习开、3对3轮配对测量中，可见语义差异0，峰值 RSS
最大62,192→50,004KiB，菜单 p95 8.097→7.904ms、p99 8.972→8.743ms。
这是原型对父分支的局部收益，不是生产生成物的最终测量，更不是对 v2.0.0
的1.20倍预算合格证明。其他 librime 版本和真实平台仍需独立验收，联合功能
默认关闭；不得据此宣布正式发布条件满足。
