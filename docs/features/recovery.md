# recovery — Recovery reservations, recipient events and retries

## Boundary

The caller owns error repair. Preserve reservation deadlines, attempt limits and downstream dependency blocking. Event acknowledgments are recipient-specific. Execution repair recomputes inferred input dependencies, retains explicit dependencies and the original output storage ID, and rejects self, transitive and workflow dependency cycles before committing.
An exact consumed-lease replay returns its durable receipt even while admission is fenced; it creates no new revision or execution. A changed request still conflicts and a new recovery mutation remains fenced.
Research Job readbacks expose each Unit's recovery state and reason. Recovery
actions target the execution ID and reject a logical Job ID with 409
`recovery_unit_required`. MCP tool errors include the application status for
clients that reconcile definite rejection separately from an unconfirmed reply.

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Recovery action | `rust/crates/core/src/jobs.rs#recover` | Job + actor + claim/progress/resume/defer arguments | Incident/lease/attempt transition or conflict; caller edits do not auto-run a new unapproved scope | api, queue |
| Deadline | `rust/crates/core/src/execution.rs#next_work` | DB + clock | Expired reservation no longer blocks unrelated Jobs; dependent Jobs remain ineligible | queue |

Public actions and input schemas are in the generated [API reference](../reference/README.md). This block owns: `recovery.claim`, `recovery.progress`, `recovery.resume`, `recovery.defer`, `recovery.reactivate`, `events.list`, `events.ack`.

## Sources and contracts

- [ops/recovery.py](../../ops/recovery.py) — retained Python compatibility reference.
- [rust/crates/core/src/jobs.rs](../../rust/crates/core/src/jobs.rs) — native implementation / contract.
- [rust/crates/core/src/query.rs](../../rust/crates/core/src/query.rs) — native implementation / contract.
- [ops/access.py](../../ops/access.py) — retained Python compatibility reference.

## Change review

When transitions change, review native jobs.rs/recover, execution.rs/next_work, domain events, recipient events and query projections. Test lease concurrency, expiry, resource return and shared recovery limits.

Related blocks: [identity](identity.md) · [execution](execution.md).

## Verification

- [tests/ops/test_job_lifecycle.py](../../tests/ops/test_job_lifecycle.py)
- [tests/ops/test_worker_recovery.py](../../tests/ops/test_worker_recovery.py)
- [tests/rust/application.rs](../../tests/rust/application.rs)

Acceptance outside these fixtures:
- Actual external-agent reactivation and duplicate callback effects belong to client acceptance testing.
