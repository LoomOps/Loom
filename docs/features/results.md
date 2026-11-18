# results — Artifact registration, indexing, lineage and metadata contracts

## Boundary

Distinguish execution completion from indexing completion. Ownership changes must not overwrite original artifact or execution provenance.
File hashing, retained inventory and storage-marker reads reject symlinks and special files without waiting for a FIFO writer. Marker metadata is bounded to 64 KiB. Registered path/media descriptors close on exec; they do not become inherited producer permissions.

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

Acceptance outside these fixtures:
- Verify large artifacts and external filesystem failures against actual storage.

## Current display names

Output names are presentation curation. Native result reads, current listings,
Item metadata (including an exact source revision) and Dashboard views use the
current label without rewriting the accepted manifest or source definition.
Historical source reads retain the recorded label. A parent Job title and Task
membership do not replace an output name. Label changes keep native aliases,
source hashes, file bindings and scientific revisions unchanged.

## Dataset lineage and field semantics

`dataset_lineage.rs` owns additive exact Dataset relations. `results.link_dataset`
previews a source-bound claim and applies it with a matching digest and request
key. Endpoints must be exact Dataset revisions; registered component evidence,
transform type, self-links and cycles are checked. The operation preserves source
manifests and operator attribution. It validates structural provenance, not the
scientific truth of a submitted transform. Native producers declare per-output
`lineage` against frozen Dataset inputs; generic `consumes` remains separate.
Result reads, Item graph and bounded Dashboard projection expose these edges.
Historical mirrors share a browse key only for the same original Dataset ID and
equivalent full JSON source content, including scientific metadata and component
hashes. Normalize JSON formatting and the known publication/catalog binding
envelopes; preserve publication state. Ignore generated Artifact IDs only when
their paths have hash-bound locations. Unknown envelope fields remain significant.
This does not merge Item identities, source revisions, locations or DB rows.

Scientific extraction distinguishes stored Dataset fields, observer inputs and
policy conditioning. An observer-source string cannot imply language storage or
stored depth. Recorded BC eligibility and positive coverage retain false/zero.
Extractor v2 suppresses the ambiguous v1 dynamic-instruction alias and affected
input claims even when a captured source is offline, preserving unrelated facts.
Refreshing affected captures is additive and cannot rewrite historical bodies.


## Dataset browsing organization

`dataset_organization.rs` owns source-bound current browsing assignments. The
existing Task is the family; a stable ID/label path supplies at most 12 subgroup
levels. Organization never adds derivation edges. `results.organize_dataset`
requires an exact Dataset, an already linked Task, preview digest and idempotent
apply key. Reassignment retains prior annotation history and rejects conflicting
labels for the same active path. Original manifests and Task links are unchanged.
New Dataset outputs may declare `metadata.research.organization`. Native result
reads and Dashboard DTOs expose effective organization with its authority.
`dataset-browser.js` renders families, subgroups and an iterative multi-generation
lineage tree. Parent links across filtered or unloaded groups stay in details.

## Metadata templates

`metadata_templates.rs` owns shared immutable formats in `catalog_sources`, exact
Profile definition pins and additive source-bound adoption in `rust_annotations`.
See [the implementation plan](../metadata-templates/PLAN.md) and the server
[metadata guide](../../loom_use/metadata.md). New Component/standalone Profile
registrations freeze selected/default templates; already accepted Profiles retain
their validation contract. Publication validates templates in addition to existing
metadata_schemas, prefixes an unambiguous recorded robot, and preserves the exact
format reference. Typed v3 imports follow the same schema and naming boundary.

Template checks enforce structure, display value types and population bounds;
they cannot establish scientific truth. Existing adoption is explicitly an
operator declaration, retaining source digest, evidence, previous annotation and
original execution. Conflicting previews fail. Catalog list/detail, Item/search
and result reads expose current robot/template context without changing accepted
source bodies. Legacy extraction suppresses object/episode-array counts and
oversized compact facts, including old captured facts, while retaining valid v1/v2
captures. Original metadata stays in Sources.

## Reporting contracts

`reporting.rs` owns immutable contract/semantic-definition resources, source-bound
adaptations, frozen Profile coverage, scientific admission and verified report
publication. New scientific Profiles declare coverage and reporter capabilities;
legacy scientific Profiles cannot accept new Jobs after cutover. Accepted Jobs
retain their original contract. Publication verifies hashes, producer fences,
schemas, exact inputs, populations, SDG membership and evaluation predicates before
committing scientific records. A publication error does not restart computation.
Read APIs verify bytes and page report fields. Built-in and registered formats
share Item identity, versioned discovery and idempotent registration. Excluded,
missing and zero remain distinct. Distribution renderers consume typed numeric
points, category counts or precomputed bins, and matrix cells with explicit
admitted and assigned counts. Evaluation summaries preserve the published case
population and the pinned criteria. See [reporting](../../loom_use/reporting.md).
