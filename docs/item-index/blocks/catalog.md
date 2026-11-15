# Catalog — Item/index impact and detailed work

Status: proposed. Impact: **Direct**. No runtime change is made by this plan.

## Observed baseline

Dashboard currently receives a full catalog, while result listing iterates current native and historical records. Existing native IDs and historical snapshot references remain distinct. There is no shared file/log search projection.

Reviewed source:

- [rust/crates/core/src/catalog.rs](../../../rust/crates/core/src/catalog.rs)
- [rust/crates/core/src/query.rs](../../../rust/crates/core/src/query.rs)
- [rust/crates/core/src/history.rs](../../../rust/crates/core/src/history.rs)
- [rust/crates/core/src/media.rs](../../../rust/crates/core/src/media.rs)

## Implementation / consumer review

1. Add rebuildable Item/file/edge/log projections in a separate index DB with one writer, bounded readers and atomic per-index checkpoints.
2. Store potentially large file entries in sealed source manifests plus index rows, not a core row per file or a giant dashboard entity array.
3. Implement typed query adapters, exact alias/detail resolution, structured/lexical search, keyset cursors and generation switches.
4. Validate current source/permission/tombstone versions before returning a projected hit; represent coverage, lag and extraction state explicitly.
5. Keep existing full catalog endpoint for old consumers, but make new search/bootstrap and lazy Item detail independent of it.
6. Keep observed availability and curation separate from immutable provenance and scheduler facts.

## Invariants

- Delete/rebuild of the index changes no IDs, records or original files.
- File/log documents do not inflate research catalog/Job counts.
- No request-time filesystem traversal, full-store hashing or Docker call.

## Planned acceptance and dependencies

Cases: `TX-03`, `TX-04`, `TX-05`, `TX-06`, `FILE-06`, `QUERY-01`, `QUERY-02`, `QUERY-03`, `UI-02`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [results](results.md), [api](api.md), [scenarios](scenarios.md).
