# Runtime/deployment resource policy

The runtime planner has structural work bounds, not a universal latency promise:
128 input keys, at most 128×31 exact dictionary lookups, one entry per lookup,
four alternative native queries, two inspected candidates per alternative, and a
32+8 merge head. Longer inputs bypass planning and retain native Rime behavior.
Session memory is capped at 512 strings × 256 bytes with saturating counts.
Persistent native learning has a logical update quota, not a byte-exact DB cap.
These limits do not bound internal native dictionary search or arbitrary custom
plugins; performance must be measured on the deployed frontend.

## One runtime compiled table

Both the learning provider and read-only lookup use `xhup_flow_learn`.
Its source imports the complete `xhup_flow_flow.dict.yaml`, preserving every
word/code/weight and encoder relationship. The default deployment graph therefore
needs only the static `xhup_flow` table and this one native table; separately
compiling `xhup_flow_flow.table.bin` duplicates unused content.

The historical `xhup_flow_flow.schema.yaml` compiler helper remains an owned
source file for upgrade compatibility and explicit research use, but is no longer
a schema dependency or selected scheme. Existing old compiled files are not
silently deleted; they may remain on disk until normal user-managed cleanup.
No shared user files or learning databases are removed. Fresh deployment must
not produce the unused table.

`run-deploy-audit.sh` tests the actual `rime_deployer --build` graph (no manual
dictionary construction), requires the two used tables, rejects the duplicate,
reports exact compiled bytes and sampled peak RSS, and runs both real schemas.
Focused audit fixtures likewise do not manufacture the unused compiled table.

## Local measurement (current single-table source package)

Linux x86_64, librime 1.16.1, distro Lua git20250707; isolated debug CLI build.
The byte-bound generated package passed all 14 package tests, real deployment,
Flow/Static smoke, 129-check replay with learning off/on/restart, 19 extended
checks, exact single-writer counts and quota refusal.

- Debug generation: 34.58 s wall time; 610,652 KiB peak RSS (`time -v`).
- Actual deployment: sampled peak RSS 1,048,572 KiB.
- Static table: 4,453,796 bytes.
- One native table: 39,755,260 bytes; native prism 16,019,772 bytes and reverse
  dictionary 14,022,704 bytes.
- The unused Flow compiled table is absent. All logical source vocabulary remains.

These are one observed run with concurrent local test workloads, not repeated
release-build benchmarks or universal device guarantees. No measured memory
saving equal to a removed file's size is inferred.

## Measurement and budget interpretation

Trainer consumes a build-time immutable package/cache rather than regenerating
the dictionary during management/status. Developer generation and first deployment
remain separate, potentially memory-intensive operations. No low-memory guarantee
or Windows/macOS timing result is inferred from a Linux test. Record source
revision, exact package hashes, runtime/plugin versions, CPU/RAM, build profile,
cold/warm state and repetitions alongside generator/deployer/runtime measurements.

Older numbers in performance-baseline.md are historical comparisons, not a
current release budget or proof of absence of regression. The final release
report must use current byte-bound artifacts and measured tails/resource peaks;
do not tune a pass threshold after seeing one favorable run.
