# catalog — Catalog, preserved records and file access

## Boundary

Distinguish historical originals from current display overlays. Never assume missing files are valid artifacts.
Common IDs are decimal strings. Native identities and historical snapshots remain exact; derived search never owns Job transitions. File identity survives index deletion in authoritative sealed manifests.

The current native service is implemented in `rust/`. Python files in this block may be retained web/administrator integrations or compatibility references; they do not supersede the native transactional handlers. See [implementation boundaries](../IMPLEMENTATION.md).

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Dashboard projection | `rust/crates/core/src/catalog.rs#dashboard_catalog` | DB, actor, instance config, storage roots | Current catalog including Owner overlay and availability facts | dashboard, api |
| Artifact read | `rust/crates/core/src/media.rs#read` | Registered media index/key + allowed storage | Bounded allowed file stream or refusal; no arbitrary host path read | dashboard, api |

Public actions and input schemas are in the generated [API reference](../reference/README.md). This block owns: `results.search_items`, `results.get_item`, `results.resolve_item`, `results.item_relations`, `results.item_files`, `results.get_file`, `results.read_file`, `jobs.log_streams`, `jobs.read_log`, `jobs.search_logs`, `system.dashboard_summary`, `system.search_status`, `system.rebuild_search`, `system.catalog_task`, `jobs.prune_logs`, `system.dashboard_page`, `system.dashboard_activity`, `system.dashboard_entity`, `system.prune_search`, `system.retry_catalog_task`.

## Sources and contracts

- [ops/dashboard_read.py](../../ops/dashboard_read.py) — retained Python compatibility reference.
- [ops/history_store.py](../../ops/history_store.py) — retained Python compatibility reference.
- [ops/job_history.py](../../ops/job_history.py) — retained Python compatibility reference.
- [ops/research.py](../../ops/research.py) — retained Python compatibility reference.
- [rust/crates/core/src/catalog.rs](../../rust/crates/core/src/catalog.rs) — native implementation / contract.
- [rust/crates/core/src/media.rs](../../rust/crates/core/src/media.rs) — native implementation / contract.
- [rust/crates/core/src/query.rs](../../rust/crates/core/src/query.rs) — native implementation / contract.
- [rust/crates/core/src/items.rs](../../rust/crates/core/src/items.rs) — native implementation / contract.
- [rust/crates/core/src/index_store.rs](../../rust/crates/core/src/index_store.rs) — native implementation / contract.
- [rust/crates/core/src/inventory.rs](../../rust/crates/core/src/inventory.rs) — native implementation / contract.
- [rust/crates/core/src/log_index.rs](../../rust/crates/core/src/log_index.rs) — native implementation / contract.
- [rust/crates/domain/src/item.rs](../../rust/crates/domain/src/item.rs) — native implementation / contract.
- [rust/crates/ops/src/indexer.rs](../../rust/crates/ops/src/indexer.rs) — native implementation / contract.
- [rust/contracts/item-schema.sql](../../rust/contracts/item-schema.sql) — native implementation / contract.
- [rust/contracts/items.md](../../rust/contracts/items.md) — native implementation / contract.
- [rust/crates/core/src/graph.rs](../../rust/crates/core/src/graph.rs) — native implementation / contract.
- [rust/crates/core/src/maintenance.rs](../../rust/crates/core/src/maintenance.rs) — native implementation / contract.
- [rust/crates/ops/src/catalog_maintenance.rs](../../rust/crates/ops/src/catalog_maintenance.rs) — native implementation / contract.
- [rust/crates/core/src/activity.rs](../../rust/crates/core/src/activity.rs) — native implementation / contract.
- [rust/crates/core/src/store.rs](../../rust/crates/core/src/store.rs) — native implementation / contract.

## Change review

When projections change, check revision refresh, detail queries, file MIME/size/path boundaries and the UI. Review the full installation doctor and step exit criteria when this feature changes; a local health response does not establish user-facing readiness.

Related blocks: [identity](identity.md) · [results](results.md) · [queue](queue.md).

## Verification

- [tests/ops/test_dashboard_resources.py](../../tests/ops/test_dashboard_resources.py)
- [tests/ops/test_file_preview.py](../../tests/ops/test_file_preview.py)
- [tests/ops/test_history_availability.py](../../tests/ops/test_history_availability.py)
- [tests/ops/test_job_history.py](../../tests/ops/test_job_history.py)
- [tests/ops/test_job_lifecycle.py](../../tests/ops/test_job_lifecycle.py)
- [tests/rust/application.rs](../../tests/rust/application.rs)
- [tests/rust/test_service.py](../../tests/rust/test_service.py)
- [tests/rust/test_item_image.py](../../tests/rust/test_item_image.py)
- [tests/rust/test_reconciliation.py](../../tests/rust/test_reconciliation.py)

Run the affected suites and `deploy/features.py check`; see [the verification matrix](VERIFICATION.md). Passing retained compatibility fixtures is distinct from exercising the native handler.

Acceptance outside these fixtures:
- Verify migrated data reference integrity separately from fixtures.
