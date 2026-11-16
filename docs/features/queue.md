# queue — Shared queue ordering, admission and preparation pipelines

## Boundary

Do not preempt running Jobs. Respect dependencies, allow concurrent preparation and never reassign resources before their return. Logical workflow queue summaries use child readiness for waiting states and distinguish execution, finalization and recovery. Accepted-key submission replays return their original Job while admission is draining; new submissions remain fenced.

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Scheduling decision | `rust/crates/core/src/jobs.rs#readiness` | Job row + DB dependency/recovery/hold state + lane | Eligibility/stage/waiting reason consumed by worker and UI | api, catalog |
| Queue mutation | `rust/crates/core/src/jobs.rs#job_action` | Waiting Job, actor, relative/absolute placement | Persisted order and change receipt; running Job not preempted | api |

Public actions and input schemas are in the generated [API reference](../reference/README.md). This block owns: `jobs.assign_owner`, `jobs.queue`, `jobs.reorder`, `jobs.hold`, `jobs.resume`, `jobs.list`, `jobs.get`, `jobs.submit`, `jobs.cancel`, `jobs.logs`, `jobs.submit_workflow`, `jobs.curate`.

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
