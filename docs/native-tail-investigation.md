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
- Native exact edges of 2–32 keys, at most one returned entry per lookup, at most
  `128 * 31` lookups; no corpus scan or corpus copy in Lua.
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
README. No stable publication is authorized.
