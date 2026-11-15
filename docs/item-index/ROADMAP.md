# Delivery sequence

Status: implementation and explicitly authorized operational acceptance. The
[implementation ledger](IMPLEMENTATION.md) records delivered boundaries; source-bound
test/image receipts and separate target receipts establish actual acceptance.

| Phase | Depends on | Concrete deliverables | Exit evidence |
|---|---|---|---|
| I0 — baseline/design | Current native candidate | Source/feature impact ledger, authority/ID decisions, failure matrix and benchmark protocol | All 19 feature dispositions mapped; planned/live contracts distinguished |
| I1 — identity and change journal | I0 | Typed Item IDs, exact aliases, compact registry, migration intents and domain change events | ID precision/clock/clone/replay tests; unchanged native requests/IDs/events |
| I2 — file inventories and index engine | I1 | Sealed file-set manifests, bounded projector, generation rebuild, file/copy metadata and lexical queries | Stable File IDs after delete/rebuild; million-file core growth bound; publication/cancel fault cases |
| I3 — log collection and retrieval | I1; I2 for text index | Nonblocking host collector capability, per-effect streams, byte cursors/segments, retention and legacy adapter | Live/restart/rotation/gap/slow-log cancellation matrix on real CPU containers |
| I4 — common API and clients | I2/I3 | Registry schemas, MCP/HTTP, native adapters, versioned guides, client capability discovery | Current published plus candidate packaged client within response limits, no new group |
| I5 — Dashboard and Scenario navigation | I4 | Search/files/logs views and exact related-record links | Real native Ops + retained relay + Firefox; filters, direct hidden links, Unicode and narrow themes |
| I6 — preservation and independent acceptance | I1–I5 | Cumulative migration/backfill/rebuild/resume, same-source images/kit, doctor and rollback | Preserved rows/files/credentials/grants; source-bound feature audit, native image CPU acceptance and scale results |
| I7 — separately requested rollout | I6 + explicit deployment request | Target readiness, drained cutover/backfill, client refresh and handoff | Per-target receipts; current identity/permissions, search coverage and real logs verified |

Inventory-backed producer output is advertised with its native publication/input
adapters and guides. Acceptance covers exact member inputs, source changes, stable
IDs and bounded publication, separately from scientific producer validation.

I2 and the I3 collector design may proceed independently after their shared identity
contract. There is one core authority and one writer per index generation; no
parallel old/new scheduler or mutation owners are allowed.

First vertical slice: submit an ordinary CPU Job -> immutable result -> one stable
File ID -> query by Scenario/name -> bounded verified file read -> index deletion ->
rebuild -> same IDs, same result/Attempt/event counts. Include a live log stream and
cancellation under a deliberately slow collector before claiming a usable release.

Each phase runs: independent source/contract review -> implementation -> semantic
and fault cases -> feature-block evidence audit -> independent counterexample pass.
A found problem revises behavior, tests and block scope before another acceptance
pass. Compilation, mock tests or a healthy HTTP endpoint alone do not close I6/I7.

No general vector search service, distributed index cluster, mandatory background
scan, scientific Python replacement, new Task hierarchy, one Job per maintenance
work unit or automatic production rollout is included.
