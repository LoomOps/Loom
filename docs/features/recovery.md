# recovery — Recovery reservations, recipient events and retries

## Responsibilities and boundaries

The caller owns error repair. Preserve reservation deadlines, attempt limits and dependent-Job skipping. Event acknowledgments are recipient-specific.

## Inputs, outputs and authoritative sources

- [ops/recovery.py](../../ops/recovery.py)
- [ops/access.py](../../ops/access.py)

The linked code/schemas define fields, enums and state transitions. This document does not duplicate them.

## Call contracts

| Boundary | Input | Result or failure | Consumers |
|---|---|---|---|
| `ops/recovery.py#action` | Job + actor + claim/progress/resume/defer arguments | Incident/lease/attempt transition or conflict; caller edits do not auto-run a new unapproved scope | api, queue |
| `ops/recovery.py#expire` | DB + clock | Expired reservation no longer blocks unrelated Jobs; dependent Jobs remain ineligible | queue |

## Related features and change impact

[identity](identity.md) · [execution](execution.md)

When state transitions change, check queue dispatch, notification events, claim leases, concurrency and expiry tests. Authoritative transitions live in ops/ops.py worker/finish and ops/recovery.py.

## Tests and failure boundaries

- [tests/ops/test_worker_recovery.py](../../tests/ops/test_worker_recovery.py)

Test sources define the concrete scenarios. `deploy/test.sh` records individual results as JSON/JUnit.

## Live validation

Actual external-agent reactivation and duplicate callback effects belong to client acceptance testing.
