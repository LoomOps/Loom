# api — Public HTTP/MCP contracts and request routing

## Boundary

HTTP actions and the MCP tools listed by the connected server invoke the same operations. An acceptance receipt returns the execution identity. Exact state reads show execution and publication progress.

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Operation dispatch | `rust/crates/core/src/store.rs#execute` | Validated group/action arguments and authenticated actor | Domain response or typed Error with code, message and HTTP status | dashboard, onboarding |
| Transport | `rust/crates/core/src/http.rs#http` | HTTP or Streamable HTTP JSON-RPC | Versioned response/errors; notifications and unsupported methods retain their transport semantics | dashboard, onboarding |

Public actions and input schemas are in the generated [API reference](../reference/README.md). This block owns: `system.ssh`, `system.users`, `system.updates`, `system.check_updates`, `system.status`, `system.docker`.

## Sources and contracts

- [ops/job_contract.py](../../ops/job_contract.py) — retained Python compatibility reference.
- [ops/mcp_docs.py](../../ops/mcp_docs.py) — retained Python compatibility reference.
- [ops/mcp_groups.py](../../ops/mcp_groups.py) — retained Python compatibility reference.
- [ops/ops.py](../../ops/ops.py) — retained Python compatibility reference.
- [ops/serve.py](../../ops/serve.py) — retained Python compatibility reference.
- [rust/contracts/job-contract.json](../../rust/contracts/job-contract.json) — native implementation / contract.
- [rust/contracts/legacy-actions.json](../../rust/contracts/legacy-actions.json) — native implementation / contract.
- [rust/crates/core/Cargo.toml](../../rust/crates/core/Cargo.toml) — native implementation / contract.
- [rust/crates/reference/build.rs](../../rust/crates/reference/build.rs) — native implementation / contract.
- [rust/crates/core/src/compat.rs](../../rust/crates/core/src/compat.rs) — native implementation / contract.
- [rust/crates/core/src/contracts.rs](../../rust/crates/core/src/contracts.rs) — native implementation / contract.
- [rust/crates/core/src/error.rs](../../rust/crates/core/src/error.rs) — native implementation / contract.
- [rust/crates/core/src/http.rs](../../rust/crates/core/src/http.rs) — native implementation / contract.
- [rust/crates/core/src/lib.rs](../../rust/crates/core/src/lib.rs) — native implementation / contract.
- [rust/crates/core/src/store.rs](../../rust/crates/core/src/store.rs) — native implementation / contract.
- [rust/crates/ops/Cargo.toml](../../rust/crates/ops/Cargo.toml) — native implementation / contract.
- [rust/crates/ops/src/lib.rs](../../rust/crates/ops/src/lib.rs) — native implementation / contract.
- [rust/crates/ops/src/main.rs](../../rust/crates/ops/src/main.rs) — native implementation / contract.
- [rust/crates/ops/src/worker.rs](../../rust/crates/ops/src/worker.rs) — native implementation / contract.
- [rust/contracts/action-docs.json](../../rust/contracts/action-docs.json) — native implementation / contract.
- [rust/crates/core/src/query.rs](../../rust/crates/core/src/query.rs) — native implementation / contract.

## Change review

When public inputs or responses change, check schemas, examples, API revision, guides and Dashboard consumers. Since ops.py contains multiple feature entry points, narrow impact to the affected functions.

Related blocks: [identity](identity.md) · [queue](queue.md) · [execution](execution.md) · [recovery](recovery.md) · [results](results.md) · [extensions](extensions.md) · [catalog](catalog.md) · [diagnostics](diagnostics.md).

## Verification

- [tests/ops/test_api_contract.py](../../tests/ops/test_api_contract.py)
- [tests/ops/test_generic_agent.py](../../tests/ops/test_generic_agent.py)
- [tests/ops/test_initialization.py](../../tests/ops/test_initialization.py)
- [tests/ops/test_job_contract.py](../../tests/ops/test_job_contract.py)
- [tests/ops/test_job_lifecycle.py](../../tests/ops/test_job_lifecycle.py)
- [tests/ops/test_mcp_docs.py](../../tests/ops/test_mcp_docs.py)
- [tests/rust/application.rs](../../tests/rust/application.rs)
- [tests/rust/codec.rs](../../tests/rust/codec.rs)
- [tests/rust/test_service.py](../../tests/rust/test_service.py)

Acceptance outside these fixtures:
- Verify actual MCP client compatibility and network disconnections separately.

Native read admission permits four active readers and two heavy catalog/file/log
readers. At most 32 read requests may execute or wait, with one three-second
slot-acquisition deadline. Queued heavy reads do not hold general slots needed
by status or Inspector telemetry. Cancelled waiters release their admission.

The action/DTO authority is `rust/crates/contracts/src/lib.rs`. The Core `contracts` module is a stable facade; handlers and transactions remain in Core. Shared lineage transform vocabulary has one authority in the contracts crate.
