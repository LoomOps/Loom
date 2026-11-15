# Additive migration, backfill, transfer and restore

Status: proposed. Existing schema 5 is the baseline; do not edit the previously
numbered native cutover migration or silently reuse its receipt as index acceptance.
A later explicit migration/schema boundary will be assigned during implementation.

## Migration inventory

| Data | Planned action | Preservation proof |
|---|---|---|
| Jobs/workflows/nodes/Attempts/Steps | Add common aliases and stream descriptors | Original IDs/specs/times/states/request keys/recipient events unchanged |
| Native result revisions | Adopt exact original manifest into inventory descriptors | Original body, file hashes, lineage and counts unchanged |
| Historical rows/bindings | Full source/kind/id/revision/snapshot mapping | No latest-row substitution or original status overwrite |
| Component/Tool/Profile source/build | Add sidecar descriptors and exact aliases | Sealed source trees, Cargo locks and accepted pins unchanged |
| Host Runtime/dependencies | Protected host snapshots and grant-qualified aliases | Separate Runtime DB/catalog authority/roots preserved |
| Existing worker/stage/ticket logs | Register only supported identities and provable byte ranges | Unknown generation/timestamp/gap remains unknown |
| Accounts/tokens/storage identities | No model migration | Byte/row/permission preservation under existing backup contract |
| Search DB | Build a new generation | No authority or scientific record created by rebuild |

Use one unique backfill mapping per exact native identity. Allocate IDs inside a
resumable durable intent/ledger and persist file-entry IDs in sealed manifests before
indexing. Restart after any chunk/DB commit must reuse assigned IDs. Mark coverage
per kind/snapshot/inventory; partial adoption must not be reported as complete.
Never backfill by invoking Job submit, re-registering a dataset or rerunning a model.

## Sequence

1. Preflight: current writer/schema/fence, configured roots/markers/ownership,
   free metadata/index/spool budget, exact Runtime grants and source inventories.
   Capture source counts and row/file hashes using bounded snapshots.
2. Stop the relevant writers for the additive schema boundary under existing drain
   rules, take verified core/config/manifest/host-catalog backups, record intent,
   and apply the new tables/compatibility version. Do not reuse old native test PASS.
3. Enable journal producers before live backfill. A consistent source high watermark
   plus idempotent aliases handles new records during bounded resumed adoption.
4. Build authoritative inventory sidecars and generation G, preserving conflicts as
   unresolved. Pure index backlog does not pause normal Jobs after schema readiness.
5. Catch up journal changes, validate current permissions, exact aliases, coverage,
   file/log references and source counts; enable search only for completed scopes.
6. Keep old APIs/IDs and previous index generation until consumer/package acceptance.
   Record source snapshot, schema/extraction versions and independent acceptance.

Runtime catalogs remain independently authoritative. Retain the last accepted exact
snapshot with the core descriptor; observe current freshness separately. Backfill
cannot assume all host revisions remain granted or that an observation is a new
Runtime installation.

## Snapshot and transfer

Preserve credentials in a private same-installation backup/rebuild under the
existing contract. Portable export/import continues to strip resident and temporary
credentials; common Item identity is not a credential/account transfer mechanism.
A new destination keeps its own installation/authentication identity. Preserved
Item origin is distinct from the destination instance ID or current machine ID.
Do not copy accounts/tokens merely because an index or Item ledger is transferred.


Classify core DB + identity ledger + sealed inventories/stream manifests + accepted
verification receipts as authoritative metadata. Copy raw research/log bytes only
according to the existing selected storage transfer/retention contract. Core
locators may need additive destination copy bindings with preserved storage origin.
An index DB is an optional acceleration snapshot, not required truth.

SQLite online backup or a quiescent verified snapshot is required; do not copy live
WAL files independently. Include index generation/checkpoint/source epoch only when
consistent. Verify a supplied index against origin/schema/descriptor hashes before
using it; otherwise rebuild. A missing index alone is not a failed data migration.
A missing authoritative inventory is a failed preservation boundary.

A deliberate transfer preserves Item IDs, aliases, origins and immutable revisions.
The existing remote worker fence remains required where configured. Search workers
cannot seize execution authority or declare old owners returned. Deliberate clone
behavior must be explicit; common Item IDs are not an invitation to merge forks.

## Rollback and rebuild

Dropping/replacing a broken derived index generation is safe after readers release
it. Rolling back authority tables/config is allowed only when the current native
migrator can prove no new records, assigned IDs, manifests, retention decisions or
copy bindings would be lost. After new writes, perform forward repair or preserve
new records; restoring an old DB to regain a green search status is forbidden.

The updater/doctor must separately report schema/identity readiness, core health,
source manifests, host collector capability and index generation/lag. Reinstallation
preserves configured roots, owner/mode/ACLs, accounts, tokens, Runtime grants and
user source revisions. Index/collector services use the existing service identity,
not Docker-group membership or a socket mounted into Jobs/Ops.
