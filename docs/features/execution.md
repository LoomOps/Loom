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

## Registered RW storage

Optional runner.storage is a unique list of data/N IDs. Profile registration checks the installation grant; executor startup rechecks IDs, the configured host roots and symlink changes. Paths are never supplied by the caller. Explicit mounts are RW at /loom/data/N and appear in request.storage. Omission preserves existing Job isolation, /work result publication and read-only pinned input mounts. system.status.data_stores reports this capability; both HTTP and MCP use the same profile schema. New installation grants all selected stores, but image updates preserve existing grants and data locations.

## Registered output storage

`ops/storage.py` owns named stores and persistent `.loom-storage.json` identities. `jobs.submit.output_storage` defaults to data/0 and freezes a non-default identity; all Attempt stages and result retries use that root. Result locators carry the selected ID and store-relative path. Registered secondary outputs are available in Dashboard files/media, results API and read-only downstream input mounts. Unknown, redirected, missing or replaced stores must never fall back to the primary. Existing Jobs without this field retain primary behavior; no DB rewrite or data move is required. Profile `storage` mounts remain separate optional shared RW access.
