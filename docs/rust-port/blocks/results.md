# Results, immutable publication and current locations — detailed design

Status: implemented development candidate; operational deployment not performed.
Concrete ownership, test boundaries and external limits: [IMPLEMENTATION](../IMPLEMENTATION.md). Phases: D1, D4, D5.

## Responsibility and model

**Authority:** loom-app publication/lineage commands + loom-storage I/O + loom-query availability.

**Typed model:** EntityRevisionId, PublicationIntent, ContentDigest, StoreId, Locator, HistoricalIdentity, AvailabilityBinding.

**Operations:** PreparePublication, CommitPublication, Reindex, ReconnectNative, BindHistoricalLocation and ExplicitPromotion.

**Persistence:** entity_revisions/entity_locations/job_annotations/result_outbox/history_records plus additive historical binding receipts; originals stay immutable.

## Reference source and consumers

- [ops/registry.py](../../../ops/registry.py)
- [ops/retain-artifact.py](../../../ops/retain-artifact.py)
- [ops/storage.py](../../../ops/storage.py)

Existing feature dependencies: identity, extensions.
The references establish old behavior, not target Rust module boundaries.

## Implementation work

1. Validate file bytes/shape/locators outside long transactions, capture exact digests and expected Attempt, then verify the same identity at publication commit.

2. Preserve typed result schema, provenance, relations and metadata validation. Generic file/schema validity is distinct from scientific count/split/Sim acceptance; profile validators own the latter.

3. Changed published bytes cannot overwrite a prior revision on reindex. Identical reindex reuses publication/events without duplicated completion or dataset counts.

4. Add verified current locations for a fully qualified historical identity, retaining recorded availability. Store marker, size/hash and access evidence are required; multiple copies are explicit.

5. Explicit import/promotion records origin lineage and does not fabricate a generation/training Job. An unknown historic hash or differing live snapshot is a conflict, not automatic approval.

6. Activity projections use recorded registration/completion times when producer creation time is absent; label each source. Do not pretend registration was dataset generation.

## Transaction and effect boundary

File publication follows the durable staging/intent/rename/fsync/receipt protocol. DB rows and completion/outbox changes are fenced to current Attempt and cancellation; no filesystem action is assumed to roll back with SQLite.

## Retained code and compatibility

Raw manifests, prior revisions, content files, exact locators and historical snapshots. Large scientific payloads remain external to the core DB.

Use additive binding tables and preserve native reconnect semantics. Runtime/Job/Scenario migrations never copy scientific payloads or reinterpret historical missing as always available.

## Acceptance cases

| Case | Trigger | Required result |
|---|---|---|
| RES-01 | Reindex finds changed bytes | Explicit failure; prior entity revision/hash remains unchanged. |
| RES-02 | Moved historical file with verified preserved copy | Add current binding, preserve original row and recorded availability. |
| RES-03 | Identical binding/publication retry | Same receipt; no new entity/Job/completion count. |
| RES-04 | Dataset has registration time only | Activity appears with truthful timestamp provenance. |

Reference regression modules (adapt moved internals to actual Rust boundaries):

- [tests/ops/test_result_pipeline.py](../../../tests/ops/test_result_pipeline.py)
- [tests/ops/test_output_contract.py](../../../tests/ops/test_output_contract.py)
- [tests/ops/test_installation_smoke.py](../../../tests/ops/test_installation_smoke.py)
- [tests/ops/test_storage_results.py](../../../tests/ops/test_storage_results.py)
- [tests/ops/test_job_lifecycle.py](../../../tests/ops/test_job_lifecycle.py)

## Exit review

Validate the cases above plus the shared [domain](../DOMAIN.md), [API](../API-CHANGES.md),
[compatibility](../COMPATIBILITY.md) and [migration](../MIGRATION.md) contracts.
Record exact source/artifact hashes and which external boundaries remain unverified.
A passing old Python suite or compiling crate alone does not close this block.
