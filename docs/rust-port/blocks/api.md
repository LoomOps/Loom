# API and MCP application adapters — detailed design

Status: proposed Rust redesign; no implemented/deployed claim. Phases: D1, D6.

## Responsibility and model

**Authority:** loom-contracts registry + loom-api transport + loom-app commands.

**Typed model:** OperationName, RequestKey, ContractRevision, TypedError, AuthenticatedContext, PageCursor.

**Operations:** One registry maps each action to a typed request/result, scope, handler and guide. HTTP and MCP invoke identical application operations.

**Persistence:** No transport-owned domain tables. Stable receipts belong to the application; guide/schema/build markers are immutable build metadata.

## Reference source and consumers

- [ops/job_contract.py](../../../ops/job_contract.py)
- [ops/mcp_docs.py](../../../ops/mcp_docs.py)
- [ops/mcp_groups.py](../../../ops/mcp_groups.py)
- [ops/ops.py](../../../ops/ops.py)
- [ops/serve.py](../../../ops/serve.py)

Existing feature dependencies: identity, queue, execution, recovery, results, extensions, catalog, diagnostics.
The references establish old behavior, not target Rust module boundaries.

## Implementation work

1. Freeze all six current groups, action schemas and valid/error fixtures from mcp_groups/mcp_docs/operate, including legacy HTTP routes and scope filtering.

2. Replace dictionary name-switch routing with a typed operation registry; generate compact/full schema references and usage examples from the same definition.

3. Implement bounded HTTP body/JSON depth/concurrency, TLS and request deadlines without treating a client disconnect as cancellation of accepted work.

4. Preserve MCP JSON and request-scoped SSE, initialize/notification/ID/header semantics and safe structured diagnostic locators. Do not flatten a tool error into successful text.

5. Add Scenario/authoring/historical-binding operations only as advertised capabilities in API-CHANGES. Compare current pinned Loom Use and external coordinator transports against the same Rust endpoint.

## Transaction and effect boundary

Handlers perform bounded decode/authentication, then call an application unit of work. They cannot execute raw SQL or choose lifecycle states. A lost reply never causes implicit external mutation replay.

## Retained code and compatibility

Existing Python client/HTTP test harness and retained web relay. A Rust MCP library is adopted only after actual wire compatibility, not to force API renaming.

Keep old operation/request codecs beside new typed DTOs. Do not increment an existing numbered release in place. Registry, guides, client and browser changes ship together.

## Acceptance cases

| Case | Trigger | Required result |
|---|---|---|
| API-01 | Same command over HTTP and MCP | Same state/receipt and equivalent safe error. |
| API-02 | Oversized/malformed/unknown DTO | Bounded rejection before durable acceptance. |
| API-03 | Response lost after committed submit | Original key returns the same Job; modified request conflicts. |
| API-04 | Old client meets additive Scenario actions | Old actions still work; new actions require live discovery. |

Reference regression modules (adapt moved internals to actual Rust boundaries):

- [tests/ops/test_api_contract.py](../../../tests/ops/test_api_contract.py)
- [tests/ops/test_mcp_docs.py](../../../tests/ops/test_mcp_docs.py)
- [tests/ops/test_job_contract.py](../../../tests/ops/test_job_contract.py)
- [tests/ops/test_generic_agent.py](../../../tests/ops/test_generic_agent.py)
- [tests/ops/test_initialization.py](../../../tests/ops/test_initialization.py)
- [tests/ops/test_job_lifecycle.py](../../../tests/ops/test_job_lifecycle.py)

## Exit review

Validate the cases above plus the shared [domain](../DOMAIN.md), [API](../API-CHANGES.md),
[compatibility](../COMPATIBILITY.md) and [migration](../MIGRATION.md) contracts.
Record exact source/artifact hashes and which external boundaries remain unverified.
A passing old Python suite or compiling crate alone does not close this block.
