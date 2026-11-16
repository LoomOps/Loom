# recovery — Recovery reservations, recipient events and retries

## Boundary

The caller owns error repair. Preserve reservation deadlines, attempt limits and dependent-Job skipping. Event acknowledgments are recipient-specific.

The current native service is implemented in `rust/`. Python files in this block may be retained web/administrator integrations or compatibility references; they do not supersede the native transactional handlers. See [implementation boundaries](../IMPLEMENTATION.md).

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

Run the affected suites and `deploy/features.py check`; see [the verification matrix](VERIFICATION.md). Passing retained compatibility fixtures is distinct from exercising the native handler.

Acceptance outside these fixtures:
- Actual external-agent reactivation and duplicate callback effects belong to client acceptance testing.
