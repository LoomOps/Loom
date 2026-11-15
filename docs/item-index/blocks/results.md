# Results — Item/index impact and detailed work

Status: proposed. Impact: **Direct**. No runtime change is made by this plan.

## Observed baseline

Results store manifests and entity_locations; historical bindings add verified copies. File preview IDs include the native identity, component, store and relative path, so they change when a copy moves.

Reviewed source:

- [rust/crates/core/src/results.rs](../../../rust/crates/core/src/results.rs)
- [rust/crates/core/src/history.rs](../../../rust/crates/core/src/history.rs)
- [rust/crates/core/src/storage.rs](../../../rust/crates/core/src/storage.rs)
- [rust/crates/core/src/media.rs](../../../rust/crates/core/src/media.rs)

## Implementation / consumer review

1. Stage stable File IDs and sealed inventories alongside existing verified immutable publication; commit only a compact descriptor and change event with the original result transaction.
2. Keep logical File, exact expected bytes and physical copy/availability as separate records. Shared hashes do not merge datasets or lineage.
3. Map current native and historical full identities; never normalize old body/manifest contents merely for indexing.
4. Reuse marker/grant/no-symlink/hash proof checks for read/reconnect. Copy/binding adoption is keyed and version-fenced, with large receipt manifests outside the writer.
5. Preserve missing/unverified/denied/offline/conflicting distinctions and explicit current versus recorded availability.
6. Keep arbitrary unregistered directories and scientific validation out of ordinary search.

Large-output schema work must also prevent new per-file duplication into legacy
`entity_locations`/embedded component arrays. Keep old records unchanged; implement
a negotiated inventory-backed producer/publication/input adapter and explicit
legacy paging response. Search alone cannot establish this end-to-end contract.

## Invariants

- No new research records, Job runs or changed artifact counts on backfill/rebuild.
- Cancellation and exact Attempt fences win publication races.
- Index data is not proof of file bytes or scientific correctness.

## Planned acceptance and dependencies

Cases: `ID-05`, `TX-01`, `TX-02`, `FILE-01`, `FILE-02`, `FILE-03`, `FILE-05`, `FILE-07`, `MIG-01`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [execution](execution.md), [catalog](catalog.md), [portability](portability.md).
