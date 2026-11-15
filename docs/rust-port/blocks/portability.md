# Portability, snapshots and writer fencing — detailed design

Status: proposed Rust redesign; no implemented/deployed claim. Phases: D1, D4, D7.

## Responsibility and model

**Authority:** loom-migrate/loom-app fencing + retained administrator transfer drivers.

**Typed model:** InstallationId, StoreId, FenceEpoch, SnapshotIdentity, PreservationManifest and MigrationReceipt.

**Operations:** Snapshot/Verify/Restore under explicit admin scope; DrainAdmission, FenceOldWriter and VerifyOwnership before transfer.

**Persistence:** All existing DBs, source revisions, Runtime grants, receipts, configured roots and instance-specific secrets.

## Reference source and consumers

- [deploy/assign-owner.py](../../../deploy/assign-owner.py)
- [ops/portability.py](../../../ops/portability.py)
- [ops/remote-handoff.py](../../../ops/remote-handoff.py)
- [ops/remote_fence.py](../../../ops/remote_fence.py)

Existing feature dependencies: identity, catalog, results, execution.
The references establish old behavior, not target Rust module boundaries.

## Implementation work

1. Preserve independent installation/store/credential identities; a restored binary is not permission to clone one host identity into another.

2. Use consistent DB backup plus matching source/config/file manifest, exact hashes and current epoch. Large scientific payload copies remain explicit, verified operations.

3. Fence old and shared helpers before new ownership and retain unresolved execution tickets. Network loss does not prove the old writer is dead.

4. Include Scenario revisions/links and authoring/build receipts in preservation manifests without rewriting historical identities.

5. Rollback keeps post-cutover data: validate schema/code readability or use a complete consistent documented restore/forward repair. Never replace just one old DB beside newer files.

## Transaction and effect boundary

Fencing and admission changes are authoritative transactions; multi-DB/file snapshots use the documented staged preservation protocol and remain unusable until independently verified.

## Retained code and compatibility

Existing explicit transfer/copy tools and private host settings, outside public source. A generic autonomous reconciler remains deferred.

Honor current migration/fence protocols and cumulative update graph. Do not erase credentials by reusing a fresh-install path.

## Acceptance cases

| Case | Trigger | Required result |
|---|---|---|
| PORT-01 | Old worker reconnects after transfer | Epoch/grant rejects mutation/launch; no dual owner. |
| PORT-02 | Snapshot has mismatched DB/files/store marker | Restore refuses readiness with exact mismatch. |
| MIG-01 | Skipped-version preserved rebuild | Credentials, registries, Scenario links and immutable pins survive. |

Reference regression modules (adapt moved internals to actual Rust boundaries):

- [tests/ops/test_remote_ownership.py](../../../tests/ops/test_remote_ownership.py)
- [tests/deploy/test_boot_readiness.py](../../../tests/deploy/test_boot_readiness.py)

## Exit review

Validate the cases above plus the shared [domain](../DOMAIN.md), [API](../API-CHANGES.md),
[compatibility](../COMPATIBILITY.md) and [migration](../MIGRATION.md) contracts.
Record exact source/artifact hashes and which external boundaries remain unverified.
A passing old Python suite or compiling crate alone does not close this block.
