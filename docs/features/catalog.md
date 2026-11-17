# catalog — Catalog, preserved records and file access

## Boundary

Distinguish historical originals from current display overlays. Never assume missing files are valid artifacts.
Common IDs are decimal strings. Native identities and historical snapshots remain exact; derived search never owns Job transitions. File identity survives index deletion in authoritative sealed manifests. Relation pages continue into incoming edges when the last outgoing edge fills a page exactly. Activity uses accepted prepare/run stage intervals; Attempt elapsed time is a compatibility fallback only when stage receipts are absent.

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Dashboard projection | `rust/crates/core/src/catalog.rs#dashboard_catalog` | DB, actor, instance config, storage roots | Current catalog including Owner overlay and availability facts | dashboard, api |
| Artifact read | `rust/crates/core/src/media.rs#read` | Registered media index/key + allowed storage | Bounded allowed file stream or refusal; no arbitrary host path read | dashboard, api |

Public actions and input schemas are in the generated [API reference](../reference/README.md). This block owns: `results.refresh_metadata`, `results.search_items`, `results.get_item`, `results.resolve_item`, `results.item_relations`, `results.item_files`, `results.get_file`, `results.read_file`, `jobs.log_streams`, `jobs.read_log`, `jobs.search_logs`, `system.dashboard_summary`, `system.search_status`, `system.rebuild_search`, `system.catalog_task`, `jobs.prune_logs`, `system.dashboard_page`, `system.dashboard_activity`, `system.dashboard_entity`, `system.prune_search`, `system.retry_catalog_task`.

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

Acceptance outside these fixtures:
- Verify migrated data reference integrity separately from fixtures.

Completed consolidation sources expose consolidated_into only while the frozen
source Attempt still matches and the accepted consolidation is complete with
ready artifacts. Research history can include support work without expanding
those source executions into separate main Jobs. A later source Attempt remains
visible and does not inherit the old consolidation's completion.

## Scientific metadata projection

`dashboard_page` and `dashboard_entity` expose `research_summary` for Datasets,
Models and Experiments. The canonical fact vocabulary and extraction paths live
in `research_metadata.rs`; the DTO is described by
[research-summary.schema.json](../../rust/contracts/research-summary.schema.json).
The same DTO is available through the existing HTTP and MCP system actions.

Accepted metadata leads. Missing fields can be recovered from registered JSON
artifacts of the exact source revision, with source identity, SHA-256 and JSON
pointer on every fact. A sibling report is eligible only for a unique Dataset or
Model of the same Job/Attempt and when its artifact map explicitly matches every
component hash of that product. Filename similarity and cached UI records are not
bindings. Recorded metadata has priority; disagreements between files are exposed
as conflicts rather than merged or averaged. Evidence pointers for record facts
refer to the effective metadata/properties object, including preserved top-level typed fields, its typed inputs,
protocol and dataset overview.

Reads use the existing no-symlink registered-location and byte-hash verifier.
At most 12 JSON files, 2 MiB each and 4 MiB aggregate are read per selected record.
No archive extraction, arbitrary path found in JSON, model loading, code execution,
recursive filesystem scan or database rewrite occurs. File access failures do not
remove the original record or turn an unverified source into a scientific fact.
Automatic enrichment runs on the selected read page, outside the mutation lane.
Explicit metadata capture is a bounded, single-record submit operation.

Physics, controller/sensor clocks, annotation source and instructions stay separate.
Model inputs carry their actual kind/shape/unit, not a generic used/unused label.
Observer geometry error, policy action error, collection yield and closed-loop task
success have distinct meanings and populations. Raw producer data and metrics are
preserved; the summary does not change scientific acceptance.
Whole-catalog compatibility/curation reads normalize accepted metadata only. They
do not scan registered files inside the mutation lane; use a bounded page or exact
Item detail to recover linked JSON context.

Legacy split count maps, nested count objects and `{id,count,frames}` rows
normalize to the same bounded train/validation/test count map before template
adoption. Zero and explicit unrecorded/not-applicable states remain distinct;
frames, member records and conflicting duplicate split rows are not counts.
Original source bodies and evidence pointers remain unchanged. Migration
verification compares the actual exact-entity and paged Split values, not only
the selected template reference.

## Persistent recovery of existing metadata

`results.refresh_metadata` previews one exact entity/historical revision. Apply
requires the returned source digest, a reason and an idempotent request key. The
service derives facts from accepted metadata and registered verified JSON; callers
cannot supply arbitrary scientific values. Captured facts, evidence and timestamps
are stored as an exact-source annotation in `rust_annotations`, independently of
the accepted manifest, history body, file bytes and display curation.

Every catalog path includes a matching materialized annotation, including the
metadata-only bootstrap. Live accepted facts lead; captured facts fill gaps. A
source-body digest mismatch or incompatible extractor version disables the saved
annotation. Stored facts remain recorded evidence when a source file later becomes
unavailable; current file availability is distinct from capture-time verification.
Re-running capture derives current sources again, not a re-stamped old annotation.
Projection-version triggers invalidate catalog caches after annotation changes.

Maintenance keys identify both action and arguments; a key cannot replay a different maintenance kind. Component/Tool build File descriptors use the accepted binary locator size and SHA-256. Existing sealed inventories remain immutable; these rules do not silently rewrite prior File identities.

Image-origin Dependency identities include the immutable Runtime image. Repeated observations of distinct base images must not alternate one current package or append redundant catalog changes; prior exact aliases remain preserved.

Workflow presentation refresh resolves `job_workflows`, not the physical `jobs`
table. Missing physical execution is not evidence that a logical workflow was
deleted. Paged discovery and exact native aliases must survive curation. An
operator can replay the existing presentation through `jobs.curate` with a new
key to repair an earlier derived tombstone without replacing its Item identity,
workflow, Attempts or accepted execution graph.

An unloaded Runtime link may supply id/revision without SHA. Resolve only one
existing immutable alias for that revision; reject conflicting aliases and keep
explicit-hash mismatch or missing revision failures. Never substitute the newest
Runtime. Exact Dashboard projection must use the resolved revision, including
older granted environments referenced by recorded Jobs.
