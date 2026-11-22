# queue — Shared queue ordering, admission and preparation pipelines

## Boundary

Respect Unit dependencies and lane capacity. Independent CPU preparation can run concurrently while an exclusive GPU Unit owns its requested devices. Wait for a Unit to return resources before reassigning them. While admission drains, accepted-key replays return the original entry and new submissions remain fenced.

Queue ordering changes the priority of waiting Units when they become eligible. A durable hold blocks subsequent claims across service restarts. Existing process and resource receipts remain authoritative for work already running.

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Scheduling decision | `rust/crates/core/src/jobs.rs#readiness` | Execution row, dependencies, recovery and hold state, resource lane | Eligibility, stage and waiting reason consumed by worker and UI | api, catalog |
| Queue mutation | `rust/crates/core/src/jobs.rs#job_action` | Queue entry, actor and placement | Persisted waiting order and change receipt | api |

See the generated [Job](../reference/jobs.md) and [Unit](../reference/units.md) references for admission, queue control, historical actions and their scopes.

## Sources and contracts

- [ops/dispatch_control.py](../../ops/dispatch_control.py) — active administrator dispatch-control helper; native execution remains in Rust.
- [ops/queue_control.py](../../ops/queue_control.py) — retained Python compatibility reference.
- [rust/crates/core/src/jobs.rs](../../rust/crates/core/src/jobs.rs) — native implementation / contract.
- [rust/crates/core/src/query.rs](../../rust/crates/core/src/query.rs) — native implementation / contract.
- [ops/job_contract.py](../../ops/job_contract.py) — retained Python compatibility reference.

## Change review

When queue policy changes, verify dispatch, catalog ordering and control capabilities with the same scenarios.

Related blocks: [identity](identity.md) · [execution](execution.md) · [recovery](recovery.md) · [results](results.md).

## Verification

- [tests/ops/test_job_lifecycle.py](../../tests/ops/test_job_lifecycle.py)
- [tests/ops/test_queue_contract.py](../../tests/ops/test_queue_contract.py)
- [tests/ops/test_queue_control.py](../../tests/ops/test_queue_control.py)
- [tests/rust/application.rs](../../tests/rust/application.rs)

Acceptance outside these fixtures:
- Actual long-running GPU workloads and power loss require separate acceptance testing.

Consolidation folding and duplicate-source claims compare the frozen source
Job/Attempt pair. A later Attempt does not inherit an older consolidation in
lists or Profile usage; Activity attributes each execution range by its Attempt.
Exact source details filter both Attempt and stage receipts and expose the
compatible `work_timeline`, without rewriting original execution times.

Nested consolidation resolves the terminal logical Job using frozen Attempt
membership. Activity and Profile usage share that mapping. Detailed source
receipts use an iterative, deduplicated traversal; callbacks alone do not suppress
a legacy Attempt execution interval.

## Historical compatibility

Historical `parent_job` curation is an additive outcome hierarchy, separate from
workflow execution membership and consolidation. Exact `jobs.get.family` reads
retain each member's own state, Attempt and Runtime pins. Workflow presentation
overlays apply to exact reads as well as the catalog. Partial presentation edits
must validate cycles against the merged existing hierarchy; explicit null clears
a presentation parent, while real workflow membership remains authoritative.
Neither grouping nor a failed follow-up rewrites the original root's state.
Physical `jobs.get` records expose `execution_membership` as `{root,node}` only
for actual workflow nodes, or null for independent Jobs. Consumers must use this
authority rather than treating `presentation.parent_job` as execution membership.

## Job and Unit projections

Typed research Jobs aggregate scheduled Units. The Dashboard Job Queue shows current outcomes, the Unit Queue shows physical work, and terminal history shows each research Job or standalone Unit once. Legacy records retain their original execution history.

An optional `forecast` on a running Unit uses the current Attempt's producer
counter. A first sample starts measurement. Estimates require observed progress
across two samples 30 seconds to 15 minutes apart. A sample older than 15 minutes
cannot support an estimate. Forecasts are presentation data. When a Job has a
single active Unit, its queue row can show that Unit's measured phase estimate.
An opted-in Profile distinguishes missing samples from unsupported reporting.
An Attempt can publish an unavailable reason and provide valid measured counters
later. The visible queue refreshes every five minutes.

Open outcomes retain their Job ID across plan revisions. Slots without an execution have no queue position or Attempt. See [outcome admission](../../loom_use/workflows.md#planning).
