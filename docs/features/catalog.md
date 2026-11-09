# catalog — Catalog, preserved records and file access

## Responsibilities and boundaries

Distinguish historical originals from current display overlays. Never assume missing files are valid artifacts.

## Inputs, outputs and authoritative sources

- [ops/dashboard_read.py](../../ops/dashboard_read.py)
- [ops/history_store.py](../../ops/history_store.py)

The linked code/schemas define fields, enums and state transitions. This document does not duplicate them.

## Call contracts

| Boundary | Input | Result or failure | Consumers |
|---|---|---|---|
| `ops/dashboard_read.py#projection` | DB, actor, instance config, storage roots | Current catalog including Owner overlay and availability facts | dashboard, api |
| `ops/dashboard_read.py#open_media` | Registered media index/key + allowed storage | Bounded allowed file stream or refusal; no arbitrary host path read | dashboard, api |

## Related features and change impact

[identity](identity.md) · [results](results.md) · [queue](queue.md)

When projections change, check revision refresh, detail queries, file MIME/size/path boundaries and the UI.

## Tests and failure boundaries

- [tests/ops/test_job_history.py](../../tests/ops/test_job_history.py)
- [tests/ops/test_job_history.py](../../tests/ops/test_job_history.py)
- [tests/ops/test_dashboard_resources.py](../../tests/ops/test_dashboard_resources.py)
- [tests/ops/test_file_preview.py](../../tests/ops/test_file_preview.py)

Test sources define the concrete scenarios. `deploy/test.sh` records individual results as JSON/JUnit.

## Live validation

Verify migrated data reference integrity separately from fixtures.

## Registered output storage

`ops/storage.py` owns named stores and persistent `.loom-storage.json` identities. `jobs.submit.output_storage` defaults to data/0 and freezes a non-default identity; all Attempt stages and result retries use that root. Result locators carry the selected ID and store-relative path. Registered secondary outputs are available in Dashboard files/media, results API and read-only downstream input mounts. Unknown, redirected, missing or replaced stores must never fall back to the primary. Existing Jobs without this field retain primary behavior; no DB rewrite or data move is required. Profile `storage` mounts remain separate optional shared RW access.

`results.historical` keeps body/locations/payload_status immutable. The additive
`current_availability` observation checks only declared storage identities and
registered target refs. A source_ref without an attached copy is `not_connected`,
not evidence that the original was deleted. Small files (up to 1 MiB) are hash
checked; larger files are present_unverified. No host-path inference, automatic
mount, Bat migration, GPU load or claim of inference readiness occurs.

`results.list` includes artifact discovery from the same active catalog snapshot
as Dashboard. Historical entries expose their exact `history_ref` and
`detail_action=results.historical`; native entries retain their existing shape.
Native IDs take precedence, duplicate historical revisions select the latest
inserted record in that snapshot, and non-artifact documents are excluded.
Imported rows and execution queues are never rewritten by this read model.

Dataset detail pages use a common property summary from `dataset_overview`, with
explicit missing/not-applicable values. Nested producer diagnostics and episode
inventories remain in lazily expanded Source metadata, preserving original evidence.

Dataset list rows and detail highlights use the same normalized facts. Present
input kinds as compact modality labels; producer timing/shape/control prose stays
in detailed source metadata. List rows omit unbounded producer summaries and show
splits when recorded. Labels stay intact as facts wrap at narrow viewport widths.
Browser regression must cover the list route as well as an individual detail page.
