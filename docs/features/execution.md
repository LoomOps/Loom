# execution — Job lifecycle, script execution and container ownership

## Boundary

Loom executes Jobs. Release resources based on executor-ticket, container and process return evidence. Never pass platform secrets or Docker sockets into Jobs.

The current native service is implemented in `rust/`. Python files in this block may be retained web/administrator integrations or compatibility references; they do not supersede the native transactional handlers. See [implementation boundaries](../IMPLEMENTATION.md).

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Runtime request | `rust/crates/core/src/execution.rs#request_for` | Pinned profile, Job params, stage, input locators | Resolved execution request file; invalid paths/inputs rejected before runtime | api |
| Worker lifecycle | `rust/crates/core/src/execution.rs#next_work` | Durable queue/config/resource receipts | Preparation/run/return/result transitions; no release without resource evidence | queue, recovery, results |

## Sources and contracts

- [executor/runtime_policy.py](../../executor/runtime_policy.py) — active administrator policy validation, used by executor-attach.py and Runtime tools; native host enforces launch policy.
- [executor/server.py](../../executor/server.py) — retained Python compatibility reference.
- [ops/execution.py](../../ops/execution.py) — retained Python compatibility reference.
- [ops/hooks.py](../../ops/hooks.py) — retained Python compatibility reference.
- [ops/job_work.py](../../ops/job_work.py) — retained Python compatibility reference.
- [ops/runtime_client.py](../../ops/runtime_client.py) — retained Python compatibility reference.
- [ops/runtime_contract.py](../../ops/runtime_contract.py) — retained Python compatibility reference.
- [ops/workflows.py](../../ops/workflows.py) — retained Python compatibility reference.
- [rust/crates/core/src/execution.rs](../../rust/crates/core/src/execution.rs) — native implementation / contract.
- [rust/crates/core/src/progress.rs](../../rust/crates/core/src/progress.rs) — native implementation / contract.
- [rust/crates/domain/Cargo.toml](../../rust/crates/domain/Cargo.toml) — native implementation / contract.
- [rust/crates/domain/src/lib.rs](../../rust/crates/domain/src/lib.rs) — native implementation / contract.
- [rust/crates/host/Cargo.toml](../../rust/crates/host/Cargo.toml) — native implementation / contract.
- [rust/crates/host/src/executor.rs](../../rust/crates/host/src/executor.rs) — native implementation / contract.
- [rust/crates/host/src/lib.rs](../../rust/crates/host/src/lib.rs) — native implementation / contract.
- [rust/crates/host/src/main.rs](../../rust/crates/host/src/main.rs) — native implementation / contract.
- [rust/crates/host/src/wire.rs](../../rust/crates/host/src/wire.rs) — native implementation / contract.
- [rust/crates/host/src/logs.rs](../../rust/crates/host/src/logs.rs) — native implementation / contract.
- [rust/crates/ops/src/log_collector.rs](../../rust/crates/ops/src/log_collector.rs) — native implementation / contract.

## Change review

When execution contracts change, verify the worker, executor, restart/idempotency behavior, cancellation, result collection and Compose mounts. Review the full installation doctor and step exit criteria when this feature changes; a local health response does not establish user-facing readiness.

Related blocks: [extensions](extensions.md).

## Verification

- [tests/ops/test_executor_boundary.py](../../tests/ops/test_executor_boundary.py)
- [tests/ops/test_job_lifecycle.py](../../tests/ops/test_job_lifecycle.py)
- [tests/ops/test_job_work.py](../../tests/ops/test_job_work.py)
- [tests/ops/test_research_workflows.py](../../tests/ops/test_research_workflows.py)
- [tests/rust/application.rs](../../tests/rust/application.rs)
- [tests/rust/domain.rs](../../tests/rust/domain.rs)
- [tests/rust/host.rs](../../tests/rust/host.rs)
- [tests/rust/test_executor.py](../../tests/rust/test_executor.py)
- [tests/rust/test_service.py](../../tests/rust/test_service.py)
- [tests/rust/test_item_image.py](../../tests/rust/test_item_image.py)

Run the affected suites and `deploy/features.py check`; see [the verification matrix](VERIFICATION.md). Passing retained compatibility fixtures is distinct from exercising the native handler.

Acceptance outside these fixtures:
- Actual Docker/GPU runtimes and power loss after container creation are outside mock coverage.
