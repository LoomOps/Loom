# File inventory and searchable copies

Status: proposed; no host filesystem has been scanned for this plan.

## Publication and stable IDs

A file set is a versioned sealed inventory for an exact ItemRevision. Each entry
contains `(file_item_id, entry_key, role, relative_display_path, media_type,
expected_size, content_evidence, origin)`. Entry keys are canonical owner-relative
identifiers; they do not change when a physical copy moves to another store.
A copy has a separate locator `(installation_origin, machine_id, storage_id, ref)`.
Resolved host paths never become global identity or portable search keys.

The platform assigns IDs during a durable publication/backfill intent. Producers
submit ordinary paths/roles/manifests and do not assign arbitrary global IDs.
The staged descriptor records assigned IDs before acknowledgement. Replaying the
same parent revision reuses that exact inventory; a conflicting inventory is a
conflict, not a new registration. Keep source inventories outside old sealed source
folders so adding catalog metadata cannot invalidate Component/Tool source hashes.

Use bounded inventory chunks (initial target <=4 MiB), a descriptor with ordered
chunk hashes/counts, and an entry-key directory for bounded lookup. Large sets are
streamed and externally sorted in a private staging area with a disk budget.
No transaction or API envelope contains a million entry objects. Core stores only
the descriptor; authoritative manifests must be backed up with it. Search rows
include `file_item_id`, parent/revision, entry key, source descriptor hash, role,
normalized search fields, content digest/size when known, and copy observations.

## Large-inventory compatibility

A scalable index is insufficient if publication still copies every member into
`entity_revisions.manifest` and `entity_locations`. Preserve all existing legacy
rows/JSON, but introduce an explicitly advertised inventory-backed output version
for new large publications. Keep one compact entity/primary-location/descriptor
record; do not dual-write a million new legacy member rows. Small legacy producer
manifests continue unchanged.

A proposed producer `loom-output-inventory/v1` supplies bounded member chunks
without global IDs. The platform validates canonical membership, all declared
paths/chunk counts and required byte evidence outside the writer, assigns stable
File IDs through its intent, and seals `loom-file-set/v1` metadata. Commit its
compact descriptor using the existing publication/cancel fences. These schema names
are proposals; the current strict result-manifest validator does not support them.

New common APIs paginate all members. A legacy get that would exceed its old
response budget returns an explicit `paging_required` with a safe inventory/detail
reference; never pretend a truncated member list is complete. Preserve legacy
Job/entity input semantics through a versioned source adapter: resolve the exact
published owner revision, primary bytes and declared inventory, then prepare/mount
only the granted package/files. No eager million-member expansion on the core lane.
Artifact readiness retains the existing required content/return verification; FTS
completion is unrelated. Activation requires a real producer -> publication ->
subsequent Job input acceptance fixture, not merely a successful search.

## Discovery sources

| Source | Registration / indexing policy |
|---|---|
| Published outputs | Original manifest and verified output package; inventory joins the existing fenced publication |
| Component/Tool source and build receipt | Existing exact source pins/artifact locator; no compiler or imports |
| Profile | Its own definition plus exact linked Component/Tools/Runtime, not copied source inventories |
| Runtime/dependency files | Only explicitly declared package/root manifests under protected host grants; no scan of image internals or dependency directories by default |
| Preserved history | Exact source/snapshot and recorded locations/bindings; unknown size/hash remains unknown |
| Existing unregistered data | Explicit administrator inventory/import with scope and provenance; do not silently make research assets |

Registering/indexing files never creates another Job, dataset, model or experiment.
Archive member/tensor/episode indexing is opt-in bounded metadata, not automatic
extraction or one Item per element. Content hashes may share an internal cache but
not access rights, file identity or scientific lineage.

Runtime/dependency roots are not implicitly exposed by ordinary read scope. The
installer may grant explicit manifest-qualified catalog roots/read capabilities
through the protected host adapter. With no grant, preserve metadata discovery and
report source bytes unavailable. No blanket dependency-root mount or arbitrary
private helper path read is introduced. Image-internal package files are not
automatically exported.

## Incremental observation and extraction

A registration event is the primary update path. Known-root scans are bounded
continuations with fair store scheduling and a per-run byte/time/entry budget.
Watch notifications only mark scopes dirty; overflow requests rescan of that scope.
No request-time recursive traversal or rehash is permitted.

Use `(root identity, relative ref, device, inode, size, mtime, ctime, verifier version)`
as a local unchanged-file optimization, never as portable identity or final proof.
Re-stat before committing evidence; file replacement during hashing yields a stale
observation and retry. Cross-store copies need exact size/hash verification under
current grants. Marker/root mismatch, permission denial, offline, missing, observed
unverified and verified states stay distinct, with check time/method recorded.

Initially extract only explicit UTF-8 text formats and selected producer metadata.
Use capped bytes/CPU/time, encoding/error flags and a truncation reason. No untrusted
imports, shell, ML model load, archive extraction, document macros or arbitrary URLs.
Binary model/HDF5/media files get names, roles, sizes and declared metadata only.
Search indexing is separate from numerical/schema/scientific validation.

## Availability, reads and removal

A result hit references an exact File Item and candidate copy; authorized file reads
reuse existing marker/path/no-symlink and content-evidence checks. Large range reads use a retained exact verification receipt and unchanged local
file stamps, or report pending verification; they do not claim a fresh whole-file
hash from a partial read. Job input acceptance keeps its existing stronger checks.
Preview tokens
are temporary address adapters, not persistent file IDs. A stale hit cannot cause
fallback to an undeclared store or arbitrary path.

Keep `recorded_availability` and `current_observation` separate. `missing` requires
a completed check of the relevant registered locator, not a partial scan, lost
permission, missing mount or unavailable index. A removed copy does not delete the
logical file or its other copies. Removing a File Item needs an explicit retained
record/tombstone policy; a search rebuild cannot delete original bytes.

See [RECONCILE](RECONCILE.md) for exact move/copy/conflict rules and
[MIGRATION](MIGRATION.md) for lost-manifest and transfer behavior.
