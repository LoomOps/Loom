# results — Artifact registration, indexing, lineage and metadata contracts

## Boundary

Distinguish execution completion from indexing completion. Ownership changes must not overwrite original artifact or execution provenance.

The current native service is implemented in `rust/`. Python files in this block may be retained web/administrator integrations or compatibility references; they do not supersede the native transactional handlers. See [implementation boundaries](../IMPLEMENTATION.md).

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Output validation/index | `rust/crates/core/src/results.rs#publish_result` | Returned Job, typed manifest, storage/config, optional prepared facts | Registry entities/metrics/lineage or processing error; compute not rerun solely for indexing | api, catalog, queue |

Public actions and input schemas are in the generated [API reference](../reference/README.md). This block owns: `results.assign_owner`, `results.list`, `results.get`, `results.reindex`, `results.reconnect`, `results.annotate`, `results.history`, `results.historical`, `results.reconcile_contract`, `results.reconcile_inspect`, `results.reconcile_plan`, `results.reconcile_get`, `results.reconcile_validate`, `results.reconcile_apply`, `results.bind_historical_location`.

## Sources and contracts

- [ops/registry.py](../../ops/registry.py) — retained Python compatibility reference.
- [ops/retain-artifact.py](../../ops/retain-artifact.py) — retained Python compatibility reference.
- [ops/storage.py](../../ops/storage.py) — retained Python compatibility reference.
- [rust/crates/core/src/history.rs](../../rust/crates/core/src/history.rs) — native implementation / contract.
- [rust/crates/core/src/results.rs](../../rust/crates/core/src/results.rs) — native implementation / contract.
- [rust/crates/core/src/storage.rs](../../rust/crates/core/src/storage.rs) — native implementation / contract.
- [rust/crates/core/src/file_set.rs](../../rust/crates/core/src/file_set.rs) — native implementation / contract.
- [rust/crates/core/src/reconcile.rs](../../rust/crates/core/src/reconcile.rs) — native implementation / contract.
- [ops/mcp_groups.py](../../ops/mcp_groups.py) — retained Python compatibility reference.

## Change review

When artifact schemas change, check producers, API/MCP schemas, queue dependency readiness, catalog projections and detail views together.

Related blocks: [identity](identity.md) · [extensions](extensions.md).

## Verification

- [tests/ops/test_installation_smoke.py](../../tests/ops/test_installation_smoke.py)
- [tests/ops/test_job_lifecycle.py](../../tests/ops/test_job_lifecycle.py)
- [tests/ops/test_output_contract.py](../../tests/ops/test_output_contract.py)
- [tests/ops/test_result_pipeline.py](../../tests/ops/test_result_pipeline.py)
- [tests/ops/test_storage_results.py](../../tests/ops/test_storage_results.py)
- [tests/rust/application.rs](../../tests/rust/application.rs)
- [tests/rust/test_service.py](../../tests/rust/test_service.py)

Run the affected suites and `deploy/features.py check`; see [the verification matrix](VERIFICATION.md). Passing retained compatibility fixtures is distinct from exercising the native handler.

Acceptance outside these fixtures:
- Verify large artifacts and external filesystem failures against actual storage.
