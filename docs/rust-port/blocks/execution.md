# Execution plans, tickets and return evidence — detailed design

Status: proposed Rust redesign; no implemented/deployed claim. Phases: D1, D3, D4.

## Responsibility and model

**Authority:** loom-domain plan/state decisions + loom-app coordinator + privileged loom-executor.

**Typed model:** JobId, PlanRevision, NodeId, StepId, AttemptId, Stage, TicketId, LaunchFingerprint, ReturnEvidence.

**Operations:** AcceptExecutionPlan, ReserveStage, LaunchStage, ObserveProgress, RequestCancellation, ObserveReturn and EnqueueFinalization.

**Persistence:** jobs/attempts/stage_runs/tickets/return receipts/finalizer receipts/result_outbox/hooks, preserving existing Job and execution identities.

## Reference source and consumers

- [executor/runtime_policy.py](../../../executor/runtime_policy.py)
- [executor/server.py](../../../executor/server.py)
- [ops/execution.py](../../../ops/execution.py)
- [ops/hooks.py](../../../ops/hooks.py)
- [ops/runtime_client.py](../../../ops/runtime_client.py)
- [ops/runtime_contract.py](../../../ops/runtime_contract.py)
- [ops/job_work.py](../../../ops/job_work.py)
- [ops/workflows.py](../../../ops/workflows.py)

Existing feature dependencies: extensions.
The references establish old behavior, not target Rust module boundaries.

## Implementation work

1. Build an explicit immutable plan from exact Profile/Tool/Runtime pins; validate DAG acyclicity/output aliases at acceptance. One requested outcome owns real nodes, not a hierarchy inferred from names.

2. Distinguish producer Step progress, independently scheduled nodes and retry Attempts. SDK observations carry per-source sequence and identity; no collector invents an earlier execution.

3. Retain private Executor grants and peer/instance verification. Build Docker argv through typed policy; initial Docker CLI adapter preserves container labels, isolation, mounts and image digests.

4. Persist create/start/cancel intent under a stable ticket before external I/O. Reconcile lost replies against the same container/process identity; unknown state keeps ownership.

5. Cancellation waits for verified process group/container return, including failures entering or leaving context. An I/O failure while recording a stage must not leak a child.

6. Apply all returns/finalization/callback observations under current Attempt/plan/epoch fences. A stale old Attempt is preserved as history without updating the current run.

7. Retry finalizers/callbacks separately from training. Emit completion only after required result facts; publication failure must not imply the container is still running.

8. Port the existing lifecycle correction tests as semantic scenarios, not Python mocks copied into Rust. Model cancellation/publication and release-owner races explicitly.

## Transaction and effect boundary

Execution decisions commit state plus durable outbox; Docker/process effects and return observations happen outside the DB transaction. Only evidence for the current owner releases a lane. Cancellation is not inferred from dropping a Rust task.

## Retained code and compatibility

Registered scientific Python code/images and wire compatibility. Native Rust control orchestration is new; old Python worker helpers are not retained as state-authority RPC.

Drain all execution/result/callback/validation owners before changing coordinator/helper. Switch the worker and its protocol-compatible Executor together; a partial Rust API cannot share a live Python state machine.

## Acceptance cases

| Case | Trigger | Required result |
|---|---|---|
| EXEC-01 | Create/start reply lost | One matching launch; no duplicate or early resource release. |
| EXEC-02 | Old Attempt finishes after retry starts | Current Attempt/state/results unchanged; old evidence remains. |
| EXEC-03 | Cancel races publication or stage-record failure | Cancel survives and process return is proven; no leaked resource. |
| EXEC-04 | Parallel nodes and thirteen preserved source executions | One logical Job, real node/Step/Attempt details and unioned active time. |
| EXEC-05 | Runtime image/store/grant identity changed | Launch rejected; no silent fallback. |

Reference regression modules (adapt moved internals to actual Rust boundaries):

- [tests/ops/test_executor_boundary.py](../../../tests/ops/test_executor_boundary.py)
- [tests/ops/test_job_work.py](../../../tests/ops/test_job_work.py)
- [tests/ops/test_job_lifecycle.py](../../../tests/ops/test_job_lifecycle.py)
- [tests/ops/test_research_workflows.py](../../../tests/ops/test_research_workflows.py)

## Exit review

Validate the cases above plus the shared [domain](../DOMAIN.md), [API](../API-CHANGES.md),
[compatibility](../COMPATIBILITY.md) and [migration](../MIGRATION.md) contracts.
Record exact source/artifact hashes and which external boundaries remain unverified.
A passing old Python suite or compiling crate alone does not close this block.
