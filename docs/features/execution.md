# execution — Job lifecycle, script execution and container ownership

## Responsibilities and boundaries

Loom executes Jobs. Release resources based on executor-ticket, container and process return evidence. Never pass platform secrets or Docker sockets into Jobs.

## Inputs, outputs and authoritative sources

- [ops/execution.py](../../ops/execution.py)
- [ops/runtime_contract.py](../../ops/runtime_contract.py)
- [executor/runtime_policy.py](../../executor/runtime_policy.py)

The linked code/schemas define fields, enums and state transitions. This document does not duplicate them.

## Call contracts

| Boundary | Input | Result or failure | Consumers |
|---|---|---|---|
| `ops/execution.py#request_file` | Pinned profile, Job params, stage, input locators | Resolved execution request file; invalid paths/inputs rejected before runtime | api |
| `ops/ops.py#worker` | Durable queue/config/resource receipts | Preparation/run/return/result transitions; no release without resource evidence | queue, recovery, results |

## Related features and change impact

[extensions](extensions.md)

When execution contracts change, verify the worker, executor, restart/idempotency behavior, cancellation, result collection and Compose mounts.

## Tests and failure boundaries

- [tests/ops/test_executor_boundary.py](../../tests/ops/test_executor_boundary.py)

Test sources define the concrete scenarios. `deploy/test.sh` records individual results as JSON/JUnit.

## Live validation

Actual Docker/GPU runtimes and power loss after container creation are outside mock coverage.
