# queue — Shared queue ordering, admission and preparation pipelines

## Responsibilities and boundaries

Do not preempt running Jobs. Respect dependencies, allow concurrent preparation and never reassign resources before their return.

## Inputs, outputs and authoritative sources

- [ops/queue_control.py](../../ops/queue_control.py)
- [ops/job_contract.py](../../ops/job_contract.py)

The linked code/schemas define fields, enums and state transitions. This document does not duplicate them.

## Call contracts

| Boundary | Input | Result or failure | Consumers |
|---|---|---|---|
| `ops/queue_control.py#scheduling` | Job row + DB dependency/recovery/hold state + lane | Eligibility/stage/waiting reason consumed by worker and UI | api, catalog |
| `ops/queue_control.py#reorder` | Waiting Job, actor, relative/absolute placement | Persisted order and change receipt; running Job not preempted | api |

## Related features and change impact

[identity](identity.md) · [execution](execution.md) · [recovery](recovery.md) · [results](results.md)

When queue policy changes, verify dispatch, catalog ordering and control capabilities with the same scenarios.

## Tests and failure boundaries

- [tests/ops/test_queue_contract.py](../../tests/ops/test_queue_contract.py)
- [tests/ops/test_queue_control.py](../../tests/ops/test_queue_control.py)

Test sources define the concrete scenarios. `deploy/test.sh` records individual results as JSON/JUnit.

## Live validation

Actual long-running GPU workloads and power loss require separate acceptance testing.
