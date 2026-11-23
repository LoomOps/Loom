# execution — Job lifecycle, script execution and container ownership

## Boundary

Loom executes Jobs. Release resources based on executor-ticket, container and process return evidence. Never pass platform secrets or Docker sockets into Jobs.

An exact read-only legacy Runtime bind may mirror a path from the instance's
primary store under a nonnumbered `/loom/data` child if the current host
administrator policy still declares that exact bind. Numbered store mounts
remain reserved for individual Job grants.

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Runtime request | `rust/crates/core/src/execution.rs#request_for` | Pinned Profile, optional Task revision, Job params, stage, input locators | Resolved execution request file with the accepted Task pin when present; invalid paths and inputs rejected before runtime | api |
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
- [rust/crates/core/src/tuning.rs](../../rust/crates/core/src/tuning.rs) — bounded workload measurements and exact-environment reuse.
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

Host memory parsing rejects malformed Unicode without panicking. Executor credentials are bounded regular files. A log collector whose metadata cannot commit is killed and reaped before returning an error.
The stage return receipt preserves the accepted Executor ticket so callers
and background collection can address the same log stream after a fast exit.
When Ops restarts, it reconciles the existing ticket before inspecting current
input copies and keeps the original stage timeout. A launch that has no
running or returned ticket still verifies the frozen inputs before starting.

Artifact registration commits together with the publication receipt, lifecycle
callback reservation and recovery closure. A worker that recovers an older
unacknowledged publication reads the committed result for the same Attempt.
An outstanding finalization or publication effect remains the sole owner for
its Attempt even when another worker slot can be filled.
Finalization retries keep the run receipt and update the exact Attempt result
when publication succeeds. Earlier failures remain in Troubleshooting.

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

Acceptance outside these fixtures:
- Actual Docker/GPU runtimes and power loss after container creation are outside mock coverage.

An upstream input `member` is optional and must be a string when present. Submission and execution snapshot reads both reject other types; an invalid selection must never silently become whole-artifact input.

Registered artifact inputs enumerate granted copies of the exact revision/member.
The worker selects a copy by its accepted digest outside the application writer lane;
missing or changed copies do not hide another valid registered copy within the
configured `catalog.read_copy_candidates` budget (default 8, maximum 32). This also applies
to `{job, entity}` inputs. A sent executor request retains its frozen payload and cannot
silently switch its input mounts. Producer result manifests must be regular files and
are read with the same bounded, nonblocking file reader as registered metadata.
