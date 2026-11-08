# portability — Preserved data, Job migration and remote ownership

## Responsibilities and boundaries

Distinguish original preservation from current presentation and retain installation identifiers. Cloned installations running concurrently require distinct identities.

## Inputs, outputs and authoritative sources

- [ops/portability.py](../../ops/portability.py)
- [ops/remote_fence.py](../../ops/remote_fence.py)

The linked code/schemas define fields, enums and state transitions. This document does not duplicate them.

## Call contracts

| Boundary | Input | Result or failure | Consumers |
|---|---|---|---|
| `ops/portability.py#copy_verified` | Source/destination + expected file identity | Verified copy or explicit failure; does not reinterpret historical recipes | installation |
| `ops/remote_fence.py#apply` | DB, actor, action and ownership request | Ownership transition/receipt or refusal; controls real execution ownership | execution |

## Related features and change impact

[identity](identity.md) · [catalog](catalog.md) · [results](results.md) · [execution](execution.md)

When migration changes, check locator/storage IDs, original hashes, ownership overlays, selective exclusions and rerun idempotency.

## Tests and failure boundaries

- [tests/ops/test_remote_ownership.py](../../tests/ops/test_remote_ownership.py)
- [tests/ops/test_job_history.py](../../tests/ops/test_job_history.py)

Test sources define the concrete scenarios. `deploy/test.sh` records individual results as JSON/JUnit.

## Live validation

Verify actual cross-machine TLS/handoff and access to restored data.
