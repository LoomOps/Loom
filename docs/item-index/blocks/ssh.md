# Ssh — Item/index impact and detailed work

Status: proposed. Impact: **Associated — regression only**. No runtime change is made by this plan.

## Observed baseline

Personal SSH is a protected separate host-account service, not a Job stage or generic file-search privilege.

Reviewed source:

- [deploy/ssh_host.py](../../../deploy/ssh_host.py)
- [rust/crates/core/src/query.rs](../../../rust/crates/core/src/query.rs)
- [loom_use/operator.md](../../../loom_use/operator.md)

## Implementation / consumer review

1. Prefer authorized MCP/API file/log discovery in client instructions; retain SSH only for explicitly authorized host work not covered by those contracts.
2. Do not add search/indexer to users sudo/Docker groups or use personal SSH to bypass a denied source locator.
3. Preserve account IDs, host-key handling, revocation and existing helper paths. No SSH service/policy change is scheduled for this feature.

## Invariants

- MCP read access does not imply shell/filesystem access.
- Indexing does not provision or copy personal accounts.
- No arbitrary shell command in search extraction/reconcile.

## Planned acceptance and dependencies

Cases: `AUTH-03`, `MIG-01`, `UI-01`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [identity](identity.md), [onboarding](onboarding.md).
