# Planned verification and measurable acceptance

Status: all cases below are **planned**, not executed feature passes. The previous
native candidate's 812 tests validate its baseline, not this new Item/index model.
No operational data or host state is needed to write or validate this plan.

## Case matrix

| Case | Boundary / trigger | Required evidence |
|---|---|---|
| ID-01 | 128-bit generation, concurrent burst, clock rollback/restart | Unique IDs, canonical decimal string round trip; recorded time not guessed from ID order |
| ID-02 | JS precision, leading zeros, negative/overflow/numeric JSON | Exact IDs through Rust/MCP/browser; invalid inputs rejected without rounding |
| ID-03 | Native ID reused in another kind/origin/snapshot | Exact alias disambiguation; no cross-kind/current-history adoption |
| ID-04 | Transfer/clone/backfill restart | Same preserved IDs; conflicting origins rejected; no allocation by a read |
| ID-05 | Equal file bytes/different outputs and copy relocation | Distinct logical files; stable moved/copied File ID and lineage |
| TX-01 | Crash before/after manifest fsync/rename/core commit | Original intent reused or orphan quarantined; no duplicate scientific record |
| TX-02 | Cancel or stale Attempt during inventory publication | Existing cancellation/fence behavior preserved; no searchable uncommitted output |
| TX-03 | Index commit/reply loss/journal redelivery | Atomic per-index checkpoint and idempotent documents; no recipient ACK/event |
| TX-04 | Index corruption/disk-full/backlog | Normal Job cancel/status/return work; truthful index unavailable/behind |
| TX-05 | Rebuild with concurrent writes, source manifest pin and generation switch | Complete snapshot + replay, stable IDs and no cross-generation cursor drift |
| TX-06 | Journal compaction and old authority/index restore | Retained reconstruction checkpoint; stale generations rejected, no lost IDs |
| FILE-01 | Multi-file result/source/build inventory | Exact parent/revision/member lookup; unchanged sealed source and native counts |
| FILE-02 | Large file changes during hash/path replacement | Stale proof rejected; no old digest bound to new bytes |
| FILE-03 | Marker mismatch, offline store, missing, permission denial, partial scan | Distinct truthful observations; no false deletion or fallback root |
| FILE-04 | Source metadata/extraction failure, unsupported binary/encoding | Metadata search remains; explicit extraction state, no imports/unbounded reads |
| FILE-05 | Corrupt/missing inventory but valid search row | No authoritative reconstruction from guessed names; bounded scope failure |
| FILE-06 | Million-file inventory | Core contains bounded descriptor rows, streamed chunks and no whole-set API array |
| FILE-07 | Inventory-backed output with a real producer -> downstream Job input | Compact core rows/descriptor, complete paginated members, preserved native input/return checks and explicit legacy paging error |
| LOG-01 | Live output before return, bounded read/tail | Actual native HTTP/MCP retrieves bytes; source/index high watermarks distinct |
| LOG-02 | Slow/large log collector beside cancel/status/return | Host authority lock not held across capture; returned owner released promptly |
| LOG-03 | Same-Attempt finalizer/hook retries | Separate effect generations/streams; legacy offset history not overwritten |
| LOG-04 | Collector/host/Ops restart and duplicate source reads | No duplicate bytes; stable stream IDs; gaps recorded if continuity unprovable |
| LOG-05 | Rotation/truncation/equal timestamps/partial UTF-8/CRLF/binary bytes | Explicit source generation and exact raw-byte ranges; no fake chronology |
| LOG-06 | Source cleanup/retention expiry/disk-full | Seal/retention receipts, index body purge and preserved minimal gaps; no Job failure |
| AUTH-01 | Revoke/change principal/grant after index or cursor creation | No stale labels/snippets/facets/counts or cross-principal cursor reuse |
| AUTH-02 | Private diagnostics/credentials/recipient events | Absent from ordinary search, related edges, snippets and aggregates |
| AUTH-03 | New Item ID supplied without underlying source access | Existing source scope/path/store/ticket checks enforced |
| QUERY-01 | Structured/lexical/Unicode/Korean/path punctuation queries | Expected typed hits and bounded exact/prefix/substring behavior |
| QUERY-02 | In-place update counterexample, new FTS corpus data, page mutation/rebuild/version GC/expired cursor/long expression | MVCC as-of pages and deterministic rank, or explicit scope/history expiry; no skipped/duplicate hit or full scan fallback |
| QUERY-03 | Missing/stale/partial index and exact native reads | Honest status/coverage; native get/Job control remains available |
| QUERY-04 | Oversized metadata/log pages and envelope/base64 overhead | Responses below actual published-client limit; full data addressable by pages |
| SCN-01 | Same label/different kinds/revisions, typed profile/historical links | Exact Item graph and no association with unrelated records |
| SCN-02 | Archived Scenario or unassigned legacy context | Explicit exact context remains readable/reusable; no inferred pin or execution gate |
| SW-01 | Runtime grant removal, stale observation, duplicate package names/scopes | Exact origin/revision preserved; no dependency installation or scientific readiness invented |
| SW-02 | Legacy Python and new Rust packages | Unchanged source/build/Runtime pins; indexing never compiles or imports |
| MIG-01 | Skipped upgrade/backfill response loss and resume | Existing users/tokens/IDs/row hashes preserved; one adoption per exact ref |
| MIG-02 | Index optional transfer but missing authoritative manifest | Rebuild works only with valid authority; missing manifest prevents false ready |
| MIG-03 | New records/IDs/bindings/retention after cutover | Old-DB rollback refused without losing new metadata |
| UI-01 | Search -> Item -> file/log -> exact native detail | Native Ops + retained relay, no injected markup or arbitrary forwarding |
| UI-02 | Research/Operations/history/source filters + queue | Actual queue occupants remain; internal file/log docs do not inflate main catalog |
| UI-03 | Light/dark 1440/500px and long names/IDs | No overflow, boilerplate or misleading empty-result success |
| VERIFY-01 | Unmapped new source/test or stale/skipped suite evidence | Feature verifier rejects readiness; baseline passes not reassigned |

## Harness ownership

Pure ID/alias/state rules belong in Rust domain/application tests. Native SQLite,
manifest and source-byte tests use disposable real data and fault injection at
actual commit boundaries. Protected host/collector cases use real bounded CPU
containers and actual delayed readers, not only a mocked Docker string.
Independent Python harnesses exercise actual native HTTP/MCP with the published
client and a candidate packaged client. Firefox uses the real native service and
retained Dashboard relay. Administrator unit/systemd/provider tests remain mocks;
clean-host/reboot/provider/GPU acceptance is separately reported.

Add appropriate cases to existing `tests/rust`, browser and deployment suites;
register new source/tests and consumers in the feature catalog at implementation.
If search merits a new feature entry, assign it once rather than duplicating source
ownership across catalog/api/results. Tests may support multiple feature blocks.
The compiler artifact/test listing must still match actual executed Rust case names.

## Scale protocol and proposed budgets

Use synthetic fixtures and identical durability/auth/CPU/memory settings. Record
source/image IDs, SQLite/FTS versions, storage medium, warm/cold state, query mix,
source/index byte counts, write/checkpoint throughput, p50/p95/p99, memory and lag.
Do not reuse old list benchmarks as evidence for this feature.

CI: 10k Items, 100k file entries, >=10 MiB logs plus concurrent metadata mutation,
projection/rebuild and cancellation. Separate opt-in scale run: 100k Items, 1m file
entries and 10m bounded log-search documents with raw-byte sizes explicitly stated.
Generate/stream fixtures within a selected disk budget; document sampling honestly.

Initial design budgets on a recorded 2-CPU/1-GiB service envelope: warm selective
metadata/path search p95 <=250 ms; scoped log search p95 <=1 s; 256-KiB range read
p95 <=250 ms when source is local and ready; core cancel/status p95 regression
<=10% or 20 ms, whichever is larger, under background indexing. Index workers have
explicit memory/IO concurrency limits; evaluate peak rather than idle RSS.
These are proposed acceptance targets to calibrate in I0/I1, not measured results
or a promise independent of storage/query selectivity. Query-too-broad, timeout,
lag and truncation outcomes must be recorded, not removed from latency statistics.

Measure cold rebuild time, peak temporary disk/WAL, index-to-source size ratio,
backlog catch-up, deletion/retention cost and reader starvation. Choose a bounded
stale-index response when a scope cannot meet its budget; never weaken durability,
auth, source validation or Job ownership to make a benchmark pass.
