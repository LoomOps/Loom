# Recovery incidents, leases and retry policy — detailed design

Status: proposed Rust redesign; no implemented/deployed claim. Phases: D1, D4, D6.

## Responsibility and model

**Authority:** loom-domain recovery policy + loom-app fenced recovery commands.

**Typed model:** IncidentId, LeaseToken, LeaseDeadline, RetryBudget, FailureStage, RecoveryMode, AttemptRevision.

**Operations:** Claim, Progress, ResumeExecution, RetryFinalization, Defer and Reactivate under the existing live action contract.

**Persistence:** Existing recovery rows, audit, job_revisions, attempt_results and result_outbox; lease secrets remain private.

## Reference source and consumers

- [ops/recovery.py](../../../ops/recovery.py)

Existing feature dependencies: identity, execution.
The references establish old behavior, not target Rust module boundaries.

## Implementation work

1. Use separate recovery pending/claimed/executing facts and execution/publication facts. A running recovery Attempt stays running even with an old failed previous_result.

2. Preserve shared automatic/agent budget and configured retry reasons/backoff. Do not grant a second budget or restore a deprecated client permission by default.

3. Resume execution against the original submission identity and new accepted revision. Archive old result evidence and clear only the current projection.

4. For result_failed, reject execution-input patches and retry only the frozen publication/finalizer path on the same Attempt; no admission/prepare/training repeat.

5. Claim/progress/resume/defer validate current actor, lease, deadline and Job/Attempt/epoch; late progress cannot reclaim a newer lease. Expose uncertainty rather than invent a repair success.

6. Keep operator pause, explicit hold and agent recovery separate. Recovery APIs cannot secretly remove a user pause; client access_mode only restricts upstream authority.

## Transaction and effect boundary

Lease/budget/revision changes and recovery event commit atomically. Any required external observation is fenced and revalidated before the next command. Never expose lease secrets through public errors or catalog snapshots.

## Retained code and compatibility

Existing API capability discovery, lease deadlines and client/external coordinator no-replay policy. No resident supervisor or agent-ACK execution dependency is added.

Preserve all incident IDs/tokens/deadlines/budgets and prior Attempt history. Unknown or incompatible leased work prevents cutover rather than clearing it.

## Acceptance cases

| Case | Trigger | Required result |
|---|---|---|
| REC-01 | Finalization retry after successful execution | Same Attempt and frozen inputs; training never restarts. |
| REC-02 | Original submit replay after recovery patch | Original Job receipt; changed request conflicts. |
| REC-03 | Executing recovery with prior failure | Current run status shown, history preserved; no duplicate repair alert. |
| REC-04 | Lease expires beside operator pause | Lease releases only its reservation; pause remains. |

Reference regression modules (adapt moved internals to actual Rust boundaries):

- [tests/ops/test_worker_recovery.py](../../../tests/ops/test_worker_recovery.py)
- [tests/ops/test_job_lifecycle.py](../../../tests/ops/test_job_lifecycle.py)

## Exit review

Validate the cases above plus the shared [domain](../DOMAIN.md), [API](../API-CHANGES.md),
[compatibility](../COMPATIBILITY.md) and [migration](../MIGRATION.md) contracts.
Record exact source/artifact hashes and which external boundaries remain unverified.
A passing old Python suite or compiling crate alone does not close this block.
