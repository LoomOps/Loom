# onboarding — User guides, version identification and client entry

## Responsibilities and boundaries

The running server is authoritative for guides. Updates do not change personal instructions or active Job pins. Distributable guides, document titles and generated MCP/API reference descriptions use English; Dashboard UI localization remains independent.

## Inputs, outputs and authoritative sources

- [ops/onboarding.py](../../ops/onboarding.py)
- [loom_use/entry.md](../../loom_use/entry.md)

The linked code/schemas define fields, enums and state transitions. This document does not duplicate them.

## Call contracts

| Boundary | Input | Result or failure | Consumers |
|---|---|---|---|
| `ops/onboarding.py#bundle` | Actor, client-known revision, current schemas/contracts | Version-bound guide package and update identity; no automatic credential issuance | api, installation |

## Related features and change impact

[identity](identity.md) · [api](api.md)

When usage contracts change, update loom_use documents, bundle inventory, revisions and the stdio client.

## Tests and failure boundaries

- [tests/ops/test_onboarding.py](../../tests/ops/test_onboarding.py)

Test sources define the concrete scenarios. `deploy/test.sh` records individual results as JSON/JUnit.

## Live validation

Verify actual client reconnection and guide refresh separately.
