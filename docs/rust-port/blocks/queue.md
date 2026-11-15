# Queue, admission and resource readiness — detailed design

Status: implemented development candidate; operational deployment not performed.
Concrete ownership, test boundaries and external limits: [IMPLEMENTATION](../IMPLEMENTATION.md). Phases: D1, D4, D5.

## Responsibility and model

**Authority:** loom-domain eligibility and loom-app queue commands; loom-query occupancy projection.

**Typed model:** QueueOrder, Lane, Hold, DispatchPause, Eligibility, DependencyAvailability, ResourceOwnership.

**Operations:** SubmitOutcome, ReorderWaiting, SetHold, SetOperatorDispatchPause, ClaimNextExecution. Queries: actual occupancy, waiting eligibility/reason and terminal history.

**Persistence:** jobs, queue ordering/history, dispatch_control and dependency facts in the existing Ops DB. No UI filter state enters these records.

## Reference source and consumers

- [ops/queue_control.py](../../../ops/queue_control.py)
- [ops/dispatch_control.py](../../../ops/dispatch_control.py)

Existing feature dependencies: identity, execution, recovery, results.
The references establish old behavior, not target Rust module boundaries.

## Implementation work

1. Model eligibility as explicit facts: dependencies, publication readiness, recovery lease, operator pause/hold, available_at, Runtime validation fence and lane capacity.

2. Use the same decision for worker claims and queue reason projections. Reload the candidate/epoch at atomic claim; no preemption of a running Job to honor next/first order.

3. Preserve bounded concurrent CPU preparation and actual GPU ownership. A ready/prepared Attempt waiting for its lane is not active GPU runtime.

4. A cancelled/failed dependency cannot become available merely because stale artifact_ready remains. A verified publication from allowed finalization states is distinct from execution terminal success.

5. Queue queries include every real occupant across Research/Operations/main/support/test. Fold only explicit workflow execution membership, retain occupant counts/resources and expandable nodes.

## Transaction and effect boundary

Order mutation/hold/claim is a short transaction with expected state/attempt/epoch and authorization; external launch follows a durable intent. Eligibility does not depend on client polling, external coordinator ACK or Scenario completeness.

## Retained code and compatibility

Existing order placement semantics, operator pause keys and real lane constraints. Dashboard filter controls remain a consumer, never an admission gate.

Carry reference queue/history correction into new projections. Preserve existing orders, holds and pauses; never clean a display by deleting or terminating work.

## Acceptance cases

| Case | Trigger | Required result |
|---|---|---|
| QUEUE-01 | Reorder while another Job runs | Only eligible waiting order changes; running ownership retained. |
| QUEUE-02 | Cancelled upstream with stale artifact_ready | Downstream remains ineligible with truthful reason. |
| VIS-01 | Operations/support Job owns GPU while Research filter selected | Current queue still shows it; history filtering has no effect. |
| QUEUE-03 | Hold/pause/lease/validation fence coexist | Reasons remain distinct; expiry does not clear operator pause. |

Reference regression modules (adapt moved internals to actual Rust boundaries):

- [tests/ops/test_queue_contract.py](../../../tests/ops/test_queue_contract.py)
- [tests/ops/test_queue_control.py](../../../tests/ops/test_queue_control.py)
- [tests/ops/test_job_lifecycle.py](../../../tests/ops/test_job_lifecycle.py)

## Exit review

Validate the cases above plus the shared [domain](../DOMAIN.md), [API](../API-CHANGES.md),
[compatibility](../COMPATIBILITY.md) and [migration](../MIGRATION.md) contracts.
Record exact source/artifact hashes and which external boundaries remain unverified.
A passing old Python suite or compiling crate alone does not close this block.
