# Migration, preservation and cutover plan

The legacy repository is a reference, but installed identities and accepted records
are compatibility contracts. Redesign internal modules while preserving them.
This document does not authorize an operational migration now.

## Additive data changes

| Data | Plan | What must not happen |
|---|---|---|
| Existing Ops tables / raw specs / receipts | Preserve bytes and keys through explicit compatibility repositories | Full-table JSON reserialization or invented new Attempts |
| Scenario identities/revisions | New Ops tables with immutable revision payload/hash | Inferring scenarios from names and rewriting old records |
| Scenario links | Additive revisioned links to native or historical typed identities | Overwriting scientific lineage or multiplying datasets/experiments |
| Job Scenario pin / provenance | Atomic accepted-plan reference; old Jobs may remain unassigned | Retrospectively claiming an unrecorded protocol was used |
| Runtime DB | Preserve distinct host registrar and grants; transfer authority once | Concurrent old/new metadata writers or blanket root adoption |
| Historical availability | Additional binding/evidence table, verified locator/hash | Replacing recorded missing/unavailable history with current status |
| Effect journal / schema metadata | Versioned migrations with unique identities and expected-version fences | Clearing uncertain ownership to allow an upgrade |

DDL is specified in D1 and exercised on fixture copies before any cutover. Use
schema compatibility/minimum-writer versions and installation epoch fencing.
Query projection caches may be rebuilt; originals, credentials, leases and sealed
revisions are not caches. A schema migration is resumable/inspectable with its
own identity and cannot auto-submit a research Job.

## Filesystem/DB publication protocol

SQLite transactions do not make external file writes atomic. Stage validated bytes
under the same target filesystem, flush content, journal intent, publish without
replacing an immutable revision, fsync the directory, then commit its verified
locator/receipt. Reconcile an orphan staging file or committed publication against
its exact hash after restart; conflict is explicit. Preserve required service UID,
GID, mode and traversal/ACL semantics during atomic replacement.

For shared Runtime DB or host package registrars, use typed intent/result receipts
and expected instance grants across the protected helper. Do not claim a single
transaction spans independent SQLite databases/filesystems. See SQLite's
[atomic commit boundary](https://www.sqlite.org/atomiccommit.html).

## Storage parity and old layouts

Compare the fully merged Compose mounts, configured host/container roots, marker
IDs, API/worker/executor/Explorer/SSH mappings and effective service-user access.
Honor installations without a stored plan. Root names shown in examples are not
hard-coded destinations; dependency installs and Cargo caches use the actual
configured dependency root. Existing storage ID and result locators survive a
path correction. No active store remap; drain and preserve evidence first.

Inspect old-profile direct images/legacy runtime IDs as evidence. Offer a verified
binding plan only when image digest, exact dependency/environment identity and
instance grant agree. `Unrecorded` is not `missing`. Do not silently convert an
old image to a newly observed environment or alter accepted Profile/Job pins.

## Cumulative release migration plan

Each immutable release declares prerequisites, data/contract changes, idempotent
scripts, expected ownership, dry-run output, restart scope, preserved identities,
verification and rollback limits. Resolve all required migration edges when
skipping versions; detect gaps/conflicts instead of executing arbitrary release
text. Administrator guidance and scripts ship together in the verified kit.
Loom Use refresh never runs these administrative actions automatically.

Rebuild/reinstall is a supported fallback only with a verified preservation manifest
covering accounts, tokens, installation/storage identities, registries, source pins,
DBs, Runtime grants and generated configuration. It is not a clean reset. Verify
restoration independently before admission; do not replace every host with the
same cloned identity or expose private evidence in release source.

## Cutover sequence (later, separately authorized)

1. Build one source-qualified binary/image/SDK/client matrix and acceptance kit.
2. Inspect each selected installation; stop new dispatch/build validation and let
   execution, callbacks, result publication and Runtime validation owners return.
3. Fence every old writer/helper; retain unknown ownership for reconciliation.
4. Take consistent SQLite backups plus matching files/config/identity receipts.
   Use the [backup API](https://www.sqlite.org/backup.html), not a live DB-only copy.
5. Apply the cumulative migration, select matching protected helper and Rust Ops,
   then verify schema, source, storage parity and original credential/row hashes.
6. Exercise scoped authenticated API/client/native/browser/CPU acceptance; separate
   GPU/provider/reboot limits. Resume the existing queue only when ready.
7. Keep previous immutable binaries/images and a signed/checksummed preservation
   receipt. Rollback code only when the old reader/writer supports current schema;
   otherwise perform a documented forward repair or a complete consistent restore.

Never overwrite post-cutover work with an old DB just to make rollback pass.
Per-installation credentials/network/storage remain independent. No deployment is
part of the current plan-writing task.
