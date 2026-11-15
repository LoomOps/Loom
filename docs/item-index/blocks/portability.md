# Portability — Item/index impact and detailed work

Status: proposed. Impact: **Direct**. No runtime change is made by this plan.

## Observed baseline

Native migration preserves prior table hashes and refuses unsafe old-DB rollback. Snapshot tooling copies instance/extension state plus the independently owned Runtime catalog.

Reviewed source:

- [rust/crates/migrate/src/main.rs](../../../rust/crates/migrate/src/main.rs)
- [rust/crates/ops/src/ownership.rs](../../../rust/crates/ops/src/ownership.rs)
- [deploy/state_snapshot.py](../../../deploy/state_snapshot.py)
- [deploy/update.py](../../../deploy/update.py)

## Implementation / consumer review

1. Classify core identity/alias/change receipts and inventory/log manifests as authoritative; classify search DB/generation as optional acceleration.
2. Snapshot core and descriptors consistently; pin referenced manifests during capture/rebuild. Use SQLite backup, not independent live WAL copies.
3. Preserve Item IDs/origins/native refs when transferring stores/machines. Verify destination bindings and grants; do not allocate destination identities.
4. Reject conflicting clone/origin mappings and stale index epochs; rebuild rather than trusting an unrelated index snapshot.
5. Extend rollback refusal to new Item IDs, manifests, copy/retention receipts and backfill progress. Keep remote worker fencing independent.

## Invariants

- An index rebuild cannot erase or recreate authority.
- Accounts/tokens/storage IDs/source pins/history remain preserved.
- Loss of authoritative manifests cannot be concealed by surviving search rows.

## Planned acceptance and dependencies

Cases: `ID-04`, `TX-05`, `TX-06`, `MIG-01`, `MIG-02`, `MIG-03`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [installation](installation.md), [identity](identity.md).
