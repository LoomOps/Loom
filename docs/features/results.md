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
