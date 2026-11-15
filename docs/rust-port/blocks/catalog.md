# Catalog and historical projections — detailed design

Status: proposed Rust redesign; no implemented/deployed claim. Phases: D1, D5, D6.

## Responsibility and model

**Authority:** loom-query read models; explicit changes through loom-app only.

**Typed model:** CatalogRevision, NativeEntityRef, HistoricalIdentity, CurrentAvailability, PresentationOverlay, ScenarioLink.

**Operations:** Queries: catalog/detail/history/availability/Scenario graph/software composition. Mutations are separate curate/bind/promote commands, never projection side effects.

**Persistence:** Native result/job facts, immutable history imports/records, presentation and current-location overlays; derived caches have a source revision watermark.

## Reference source and consumers

- [ops/dashboard_read.py](../../../ops/dashboard_read.py)
- [ops/history_store.py](../../../ops/history_store.py)
- [ops/job_history.py](../../../ops/job_history.py)
- [ops/research.py](../../../ops/research.py)

Existing feature dependencies: identity, results, queue.
The references establish old behavior, not target Rust module boundaries.

## Implementation work

1. Build typed read models by identity, not brittle producer-name heuristics. Preserve raw original snapshots and expose recorded vs current availability separately.

2. Join Scenario links by exact target revision; preserve unassigned records and multi-Scenario reuse. Label proposed suggestions as unconfirmed, never authoritative identity.

3. Separate catalog curation from current queue and complete history. Direct authorized links and All mode must find hidden support/test/source records.

4. Fold entities/nodes only through explicit publication/membership/consolidation facts. A presentation parent alone cannot prove another Job executed its work.

5. Retain dataset modality/split/count semantics, zero/null/N/A and source metadata normalization. Historical planned/started/adopted logs do not become scientific output.

6. Use indexed bounded queries and on-demand exact inventories rather than repeatedly scanning large filesystem/package lists on every dashboard poll.

## Transaction and effect boundary

Projections use consistent committed read snapshots and never launch/repair/register work. Cached watermarks expose staleness; a cache refresh does not rewrite historical facts.

## Retained code and compatibility

Existing frontend normalization and raw scientific metadata; native/historical distinctions and file-access thresholds.

Rebuild derived indexes only after source/identity parity. Existing catalog DB content and presentation receipts remain preserved.

## Acceptance cases

| Case | Trigger | Required result |
|---|---|---|
| CAT-01 | Unknown legacy metadata and zero/N/A | Preserve originals and distinct display meanings. |
| VIS-02 | Support label with presentation parent but no membership | Job remains individually discoverable. |
| CAT-02 | Same artifact in two Scenarios | One entity identity, explicit links; no duplicate count. |
| CAT-03 | Binding or file inaccessible | Current availability reports evidence/limits without rewriting history. |

Reference regression modules (adapt moved internals to actual Rust boundaries):

- [tests/ops/test_job_history.py](../../../tests/ops/test_job_history.py)
- [tests/ops/test_dashboard_resources.py](../../../tests/ops/test_dashboard_resources.py)
- [tests/ops/test_file_preview.py](../../../tests/ops/test_file_preview.py)
- [tests/ops/test_history_availability.py](../../../tests/ops/test_history_availability.py)
- [tests/ops/test_job_lifecycle.py](../../../tests/ops/test_job_lifecycle.py)

## Exit review

Validate the cases above plus the shared [domain](../DOMAIN.md), [API](../API-CHANGES.md),
[compatibility](../COMPATIBILITY.md) and [migration](../MIGRATION.md) contracts.
Record exact source/artifact hashes and which external boundaries remain unverified.
A passing old Python suite or compiling crate alone does not close this block.
