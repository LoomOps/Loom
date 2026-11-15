# Identity, revisions and typed references

Status: proposed. Existing native ID meanings and namespaces remain intact.

## ID decision

Generate new common Item IDs as UUIDv7 128-bit values, store fixed 16-byte keys and
serialize them as canonical unsigned decimal **strings** (up to 39 digits). This
preserves the requested large-number presentation without JSON integer precision
loss. APIs reject numeric JSON Item IDs, signs, whitespace, overflow and noncanonical
leading zeros; old string native references use an explicit alias field instead.
SQLite INTEGER/FTS rowids are private 64-bit surrogates, not Item identities.

UUIDv7 provides time-oriented ordering without a shared host counter. It is not a
strict order of distributed commits: use recorded time and the per-authority change
sequence for that purpose. Clock rollback, process restart and generation bursts
must not cause reuse; select and test the pinned library's generation behavior.
Keep the existing UUIDv4 feature for legacy IDs and enable a reviewed v7 feature for
new Item IDs. No custom timestamp/worker-ID bit layout is needed.
[UUIDv7 specification](https://www.rfc-editor.org/rfc/rfc9562.html#name-uuid-version-7).

Existing Job IDs, `(owner,id,revision)` software keys, entity IDs/revisions, storage
IDs, source snapshots and credentials are **not renumbered**. New specialized calls
continue to expose their native `id`; add `item_id` and `native_ref`. Common Item
calls accept the new ID. Moving between machines preserves origin/ID; it does not
allocate destination IDs. A deliberate clone/import records a provenance link and
an explicit conflict decision instead of silently merging different origins.

## Model

| Type | Stable meaning | Mutable versus immutable |
|---|---|---|
| Item | Addressable logical research/software/execution object | Label/curation may change; kind, identity and origin do not |
| ItemRevision | Exact accepted definition/content contract | Immutable; references parent Item plus source version/hash |
| NativeAlias | `(origin, kind, canonical native identity)` -> Item/revision | Unique mapping; no name-only or latest-snapshot shortcut |
| File Item | Logical inventory entry belonging to an exact owner revision | Stable on relocation/copy; changed content requires a new owner revision or explicit mutable-source generation |
| Content evidence | Algorithm, digest, byte count and verification receipt | Immutable when verified; may be unknown, never a guessed digest |
| Copy/Location | One content-bearing store-relative copy | Observations append/refresh; withdrawal/replacement is recorded |
| LogStream Item | Attempt + stage + effect identity/generation + channel | Mutable while open; monotonic committed prefixes and explicit gaps |
| Segment | Byte interval in a stream generation | Sealed ranges immutable; retention does not reuse interval identity |

File identity is not a path, SHA, inode, blob dedup key or web-preview token.
Two outputs with equal content keep their own File Items and lineage; one File Item
may have multiple verified copies. Cross-revision identity continuity must be an
explicit relation (`supersedes`, `derived_from`, `same_logical_source`), not inference
from similar names. Whole-file hashes do not imply valid tensors or experiments.

Persist a dedicated catalog origin once (or adopt a verified existing installation
origin). Never derive it from a hostname, current machine path or credential.
Destination installation identity and immutable Item origin are separate. A
read-only replica/import may preserve IDs; an independently writable fork needs
an explicit identity/lineage decision under existing portability rules.

## Canonical references and scope

Native refs preserve the full historical `(source,kind,id,revision,snapshot)` tuple.
Software refs retain namespace/owner + ID + revision and available source hash.
Installation Profiles with an exact recorded hash become revisions; unsealed legacy
Profiles are `observed_unversioned`, not retroactively claimed to be frozen.
Runtime/dependency refs include catalog origin, exact runtime revision/grant and
recorded package scope. Dependency family aggregation by normalized name remains a
presentation projection, not proof that different providers/installations are equal.

Item kinds include existing Scenario, Job/workflow/node, result entity kinds,
Component, Tool, Profile, Runtime, declared dependency unit, File and LogStream.
Attempt/Step IDs remain typed execution references reachable through the parent;
registry adoption must preserve them rather than inventing another execution layer.
Metrics, tensors, individual log lines, transitive package inventories and archive
members are not automatically promoted to first-class Items. Explicit publication
can make a selected member addressable later. Item existence never implies dashboard
prominence, input readiness, current accessibility or additional permissions.

## Proposed authoritative records

- `catalog_items`: `item_id`, kind, origin, created/recorded time, native adapter key,
  current source version, optional tombstone. Compact domain/stream records only.
- `catalog_aliases`: full typed native identity, Item/revision mapping, evidence and
  version; unique exact key. Case-folding display/search never changes an identity.
- `catalog_revisions`: parent, immutable revision ID, native revision/snapshot,
  definition/manifest digest and creation provenance.
- `catalog_manifests`: owner revision, sealed file-set ID, descriptor locator/digest,
  entry count, schema and retention/reference state. Individual File IDs live in it.
- `catalog_log_streams`: Job/Attempt/stage/effect/generation/channel, lifecycle and
  source manifest descriptor. Identical log text does not merge streams.
- `catalog_copy_receipts`: bounded inventory/locator changes with verification and
  expected version. Bulk copies use sealed receipt manifests plus one descriptor.

The existing domain tables remain authoritative for their lifecycle. This registry
cannot set Job state or replace sealed result/software semantics.
