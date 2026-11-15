# Implementation roadmap and delivery criteria

This is the complete proposed sequence. Only the planning/audit portion of D0 is
complete in this task. No crate, endpoint, Scenario migration or Skill deployment
exists merely because it appears below. Effort estimates follow the D1 contract
and durability spikes; no date is invented from source line counts.

## Milestones

| Phase | Dependencies | Concrete deliverables | Exit evidence |
|---|---|---|---|
| D0 — independent discovery and design | None | Frozen per-file ledger, effect/contract inventory, findings, 20 block designs and requirements trace | Every input version classified; current corrections vs older candidates distinguished; plan links consistent |
| D1 — contracts and foundation | D0 review | Cargo workspace, typed IDs/states/commands, exact legacy codecs, action registry, schema plan and compatibility harness | Valid/invalid wire fixtures, hashes and original-key replay; no active mutation service; schema validator parity |
| D2 — producer SDK and package path | D1 | Rust request/progress/manifest SDK, versioned Cargo templates, source/build receipts, Rust built-in smoke packages | Build and run Tool/Component/hybrid fixtures; exact runtime/target pins; no API-side compiler or network |
| D3 — host executor and adapters | D1, D2 package contract | Protected Rust Executor, tickets/resources/returns, compatible Runtime registrar and bounded observers | Create/start/cancel/lost-reply/restart CPU fault matrix, per-instance grants, no Docker socket in Ops/Jobs |
| D4 — application writer and lifecycle | D1, D3 | One transactional Job/queue/recovery/publication authority, DAG/internal Steps, outbox/callbacks and current-Attempt fences | All inherited business-correction cases, actual kill/restart/concurrency; no stale cancellation/publication overwrite |
| D5 — software, Scenario and projections | D1; D4 for mutation integration | Component/Tool/Profile/Runtime registries, dependency representatives, Scenario revisions/links, historical bindings and queue/history projections | Many-to-many Scenario fixtures, immutable pins, unassigned historical data, all queue occupants and source drill-down |
| D6 — API, guides, Loom Use and web integration | D2, D4, D5 | Rust HTTP/MCP, generated schemas/guides, Rust-first distributed Skills and retained Dashboard changes | Reference and candidate packaged clients, external coordinator adapter, real protocol and Firefox; no stale static API list |
| D7 — packaging and migration acceptance | D3–D6 | Immutable binaries/images/SDK, cumulative scripts and admin guide, install/doctor, preserved rebuild and rollback | Fresh install, skipped-version upgrade, restart, file/DB/credential/root parity and same artifact identities |
| D8 — candidate review and authorized deployment | D7 + explicit deployment request | Operational per-target drain, backup, cutover, scoped acceptance and handoff | Independent target readiness/preservation, honest limits, no silent rollback loss or unverified success |

D2 and D3 can be implemented as independent work packages, but no parallel live
mutation owners are permitted. Read-only query prototypes may precede D4; their
existence is not permission to cut over a partial scheduler. The first usable
vertical slice is: Rust built-in package -> ordinary Profile -> one CPU Job ->
Executor return -> immutable output -> read through the retained client/browser.
It must include cancellation and a restart failure, not just a happy-path demo.

## Work package rules

For each phase: implement the designed ownership boundary, capture compatibility
fixtures, execute semantic/fault tests, then independently inspect assumptions and
counterexamples. Revise the block and fixtures when a design issue is found.
Do not copy the Python function tree or implement a generic dictionary/SQL bridge
just to keep old tests importing the same internals.

Tests stay in the language suited to the boundary: pure/application/fault rules in
Rust; independent HTTP/MCP/SDK/browser/install harnesses can remain Python/JS.
Do not delete useful cross-boundary checks merely because a Rust unit test has a
similar title. Compile native tests without production-only fault hooks.

## Cross-cutting acceptance cases

- `EXEC-01`: lost create/start reply yields one matching launch and retained owner.
- `EXEC-02`: old Attempt return cannot finish the current Attempt.
- `EXEC-03`: cancel vs publication preserves cancel and actual return evidence.
- `REC-01`: finalization-only retry never retrains or re-admits altered inputs.
- `REC-02`: original submission key replays after recovery; changed request conflicts.
- `RES-01`: changed published bytes fail reindex without overwriting prior evidence.
- `RES-02`: verified historical binding adds current availability, preserving original.
- `VIS-01`: Research/Operations/history filters never remove an actual queue occupant.
- `VIS-02`: real node membership folds; a presentation parent alone does not hide a Job.
- `SCN-01`: Scenario revision changes do not move accepted Job pins or old data links.
- `SCN-02`: one dataset/model may have several explicit Scenario uses without duplication.
- `AUTH-01`: generated new pure orchestration is Rust; hybrid SDK code is isolated/pinned.
- `AUTH-02`: Cargo package registration never executes its build script.
- `MIG-01`: skipped-version rebuild preserves credentials, stores, registries and pins.
- `MIG-02`: storage parity mismatch or unknown owner prevents a false ready report.

## Performance acceptance

D1 records reference CPU/RSS, cold start and p50/p95/p99 for queue/read/mutation/
projection under 100, 10k and 100k synthetic records plus concurrent requests.
D4/D6 repeat the same workload/hardware/SQLite durability settings. Measure event
latency, lock wait, bounded memory and cancellation/return latency; separate disk
hash/build/container overhead from the control plane. Investigate regression
before cutover; initial budgets are fixed from that baseline, not guessed here.
No benchmark may disable durability, loosen auth or execute real research Jobs.

## Deferred scope

A general autonomous reconciler, new distributed scheduler, universal plugin ABI,
full frontend rewrite, replacement of scientific Python ecosystems and automatic
production rollout are excluded. Bounded historical bindings, explicit Scenario
association and truthful runtime inspection are included in D5.
