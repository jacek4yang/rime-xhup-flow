# Bounded native boundary planning

## Current proposal

The Flow schema has one native translation/learning provider, created by
`lua_translator@*xhup_flow.native_tail`. Static primary remains separate and
learning-disabled. `flow_lookup` is a **read-only** native dictionary binding;
it has no user database or memorization callback. The old `learn` configuration
is retained for migration, but is no longer a second registered translator.
The one native provider uses the encoder-capable `xhup_flow_learn` dictionary.

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
lower it), with at most 64 committed elements of at most 256 bytes each. The native
encoder is limited to 20 elements and one homograph. Preflight conservatively
reserves `(elements + 1) * (encoder_window + 1)` updates for direct entries and
commit-history encoding. An excessive imported tick also pauses new learning.

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
