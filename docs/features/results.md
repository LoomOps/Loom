# results — Artifact registration, indexing, lineage and metadata contracts

## Responsibilities and boundaries

Distinguish execution completion from indexing completion. Ownership changes must not overwrite original artifact or execution provenance.

## Inputs, outputs and authoritative sources

- [ops/registry.py](../../ops/registry.py)
- [ops/mcp_groups.py](../../ops/mcp_groups.py)

The linked code/schemas define fields, enums and state transitions. This document does not duplicate them.

## Call contracts

| Boundary | Input | Result or failure | Consumers |
|---|---|---|---|
| `ops/registry.py#index` | Returned Job, typed manifest, storage/config, optional prepared facts | Registry entities/metrics/lineage or processing error; compute not rerun solely for indexing | api, catalog, queue |

## Related features and change impact

[identity](identity.md) · [extensions](extensions.md)

When artifact schemas change, check producers, API/MCP schemas, queue dependency readiness, catalog projections and detail views together.

## Tests and failure boundaries

- [tests/ops/test_result_pipeline.py](../../tests/ops/test_result_pipeline.py)
- [tests/ops/test_output_contract.py](../../tests/ops/test_output_contract.py)

Test sources define the concrete scenarios. `deploy/test.sh` records individual results as JSON/JUnit.

## Live validation

Verify large artifacts and external filesystem failures against actual storage.

## Registered output storage

`ops/storage.py` owns named stores and persistent `.loom-storage.json` identities. `jobs.submit.output_storage` defaults to data/0 and freezes a non-default identity; all Attempt stages and result retries use that root. Result locators carry the selected ID and store-relative path. Registered secondary outputs are available in Dashboard files/media, results API and read-only downstream input mounts. Unknown, redirected, missing or replaced stores must never fall back to the primary. Existing Jobs without this field retain primary behavior; no DB rewrite or data move is required. Profile `storage` mounts remain separate optional shared RW access.

Artifact-location repair is exposed as `results.reconnect` and HTTP POST
`/api/registry/reconnect`, with the same submit scope and storage/path/hash checks.
Discovery and generated bundle references must include every public result action.

Reindex is idempotent only for the same published output set and immutable
manifest (components/hashes, metadata, metrics and lineage). Validate existing
revisions before advertising artifact readiness; never silently skip a conflicting
existing ID. Publication and cancellation serialize on the current Attempt.
Cancellation wins even if validation raises. Result errors from stale Attempts
cannot overwrite a newer execution. Native Registry listing has no silent 250-row
cutoff; local output IDs remain addressable after Job-ID namespacing.

Successful finalizers with verified outputs have a durable per-Attempt receipt.
Index retries and reindex of an already published Attempt skip that finalizer;
they never acquire a new execution Attempt or repeat its side effects. Reindex
temporarily clears readiness until the result is verified again. Legacy published
Registry manifests also prove that the original finalizer already succeeded.
