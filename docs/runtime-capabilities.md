# Runtime capability evidence (B4 / F17)

`xhup-cli::runtime_capabilities` provides one typed vocabulary for doctor,
Trainer (typed IPC status and diagnostics), and future runtime/acceptance adapters.
Every field distinguishes Available, Unavailable and Unknown, and separately
records Filesystem, Configuration or Runtime evidence. Unknown is never PASS.

| Capability | Meaning; what does NOT prove it |
|---|---|
| `lua_payload` | Local plugin file detected; not registration or execution |
| `lua_registered` | Actual librime process registered the Lua module |
| `lua_filter_active` | Intended filter executed, not merely present in schema |
| `live_evidence_provider` | Real native lexical lookup available, not a Lua stub |
| `contextual_decoder` | Actual contextual decoder available; a candidate reranker is not one |
| `learning_configured` | Effective runtime learning configuration, not a similarly named ignored context switch |
| `storage_writable` | Real isolated write/replace probe; permission bits alone are insufficient |
| `static_fallback_usable` | Actual static scheme selection/use, not a schema filename |

Ordinary doctor is read-only. It reports file/plugin presence, leaves live facts
Unknown, and does not write to user storage to manufacture a green capability.
The existing `lua_contract_ok` bool remains a legacy **file/configuration**
contract, now labelled accordingly; its value and CLI exit status are not runtime
qualification. Windows/macOS built-in-plugin assumptions are not recorded as
observed file presence. Trainer serializes this same model in `ProductStatus` and
shows runtime evidence separately from installation integrity. A healthy install,
a plugin file, or configured filter never makes that row a runtime PASS. Its
resource-preflight result is `unverified`, not the former misleading `satisfied`.
Only explicit runtime observations can mark registration/filter execution as
observed; this still does not certify decoding, learning, or storage.

Runtime adapters can apply explicit observations. Contradictory active-filter /
unregistered-Lua observations are rejected without partially updating state.
Available filter does not imply decoder, evidence provider, learning or writable
storage. `live_flow_infrastructure_ready` covers only Lua/filter/static baseline;
it is not overall release readiness, decoding correctness or learning safety.

Current real replay asserts module registration and compiled learning-off config;
the native Lua audit proves filter behavior with option-on/off comparisons. These
are distinct tests. Remaining runtime fields and final platform evidence must be
filled by actual probes before release acceptance, not by doctor file presence.
The observation struct alone has no release-provenance authority: acceptance
still requires the exact RC/artifact/source binding from Phase 1A.
