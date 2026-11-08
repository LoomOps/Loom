# api — Public HTTP/MCP contracts and request routing

## Responsibilities and boundaries

HTTP and the six MCP groups invoke the same operations. Distinguish request acceptance from successful execution.

## Inputs, outputs and authoritative sources

- [ops/job_contract.py](../../ops/job_contract.py)
- [ops/mcp_groups.py](../../ops/mcp_groups.py)
- [ops/mcp_docs.py](../../ops/mcp_docs.py)

The linked code/schemas define fields, enums and state transitions. This document does not duplicate them.

## Call contracts

| Boundary | Input | Result or failure | Consumers |
|---|---|---|---|
| `ops/ops.py#operate` | Validated group/action arguments and authenticated actor | Domain response; ValueError/PermissionError at public boundary | dashboard, onboarding |
| `ops/ops.py#API` | HTTP or Streamable HTTP JSON-RPC | Versioned response/errors; notifications and unsupported methods retain their transport semantics | dashboard, onboarding |

## Related features and change impact

[identity](identity.md) · [queue](queue.md) · [execution](execution.md) · [recovery](recovery.md) · [results](results.md) · [extensions](extensions.md) · [catalog](catalog.md)

When public inputs or responses change, check schemas, examples, API revision, guides and Dashboard consumers. Since ops.py contains multiple feature entry points, narrow impact to the affected functions.

## Tests and failure boundaries

- [tests/ops/test_api_contract.py](../../tests/ops/test_api_contract.py)
- [tests/ops/test_mcp_docs.py](../../tests/ops/test_mcp_docs.py)
- [tests/ops/test_job_contract.py](../../tests/ops/test_job_contract.py)
- [tests/ops/test_generic_agent.py](../../tests/ops/test_generic_agent.py)
- [tests/ops/test_initialization.py](../../tests/ops/test_initialization.py)

Test sources define the concrete scenarios. `deploy/test.sh` records individual results as JSON/JUnit.

## Live validation

Verify actual MCP client compatibility and network disconnections separately.

Revision v38 adds Dashboard Admin-only token management routes documented in identity.md. They are not ordinary MCP operations. `system.status.principal` and `/api/principal` include `credential_id` for resident tokens; existing fields and six tool groups are unchanged.
