# Personal SSH access and authoring workspace — detailed design

Status: proposed Rust redesign; no implemented/deployed claim. Phases: D1, D6, D7.

## Responsibility and model

**Authority:** Retained protected account helper; read-only Rust connection discovery.

**Typed model:** PersonalAccountId, HostIdentity, AccessStatus, ConnectionMode and GrantedRoots.

**Operations:** Existing Admin-only personal account lifecycle via protected helper; read-only own-connection lookup for Loom Use/authoring.

**Persistence:** Private managed account receipts/config, current UID ownership and approved host keys; not a source repository catalog.

## Reference source and consumers

- [deploy/ssh_host.py](../../../deploy/ssh_host.py)
- [deploy/ssh_setup.py](../../../deploy/ssh_setup.py)
- [dashboard/ssh_access.py](../../../dashboard/ssh_access.py)
- [ops/ssh_info.py](../../../ops/ssh_info.py)

Existing feature dependencies: identity, network, installation.
The references establish old behavior, not target Rust module boundaries.

## Implementation work

1. Preserve same-host local mode and existing explicitly authorized personal connections; no SSH loopback or synthetic account requirement.

2. No sudo/Docker membership or platform service identity is granted for Component/Tool authoring. Work in personal drafts and register via public contracts.

3. Update optional Loom SSH Skill to prefer Rust drafts/build receipts and configured Runtime/dependency roots while preserving host-key verification.

4. Restrict helper operations to managed identities and current Admin grants; never adopt arbitrary existing accounts or execute user-provided shell commands.

5. Link Scenario/Job data only through granted filesystem/API identities, not private cross-machine chat searches.

## Transaction and effect boundary

Account operations retain their own intent/read-back receipts; registration/Job state is never edited through SSH. User files/pins survive helper replacement.

## Retained code and compatibility

Existing OS account libraries/helper and SSH transport. Rust only owns selected discovery/typed application decisions initially.

Preserve personal UID, disabled/access state, root configuration and keys. Any helper language change is independently packaged and tested under its existing privilege boundary.

## Acceptance cases

| Case | Trigger | Required result |
|---|---|---|
| SSH-01 | Same host authoring | Local user permissions used; no unnecessary SSH/account creation. |
| SSH-02 | Unmanaged existing account or unknown host key | Explicit refusal/verification requirement, no adoption. |
| SSH-03 | Rust package draft registered over MCP | Exact source/build receipt, no direct registry/DB edit. |

Reference regression modules (adapt moved internals to actual Rust boundaries):

- [tests/deploy/test_ssh_host.py](../../../tests/deploy/test_ssh_host.py)
- [tests/dashboard/test_ssh_access.py](../../../tests/dashboard/test_ssh_access.py)
- [tests/ops/test_ssh_info.py](../../../tests/ops/test_ssh_info.py)

## Exit review

Validate the cases above plus the shared [domain](../DOMAIN.md), [API](../API-CHANGES.md),
[compatibility](../COMPATIBILITY.md) and [migration](../MIGRATION.md) contracts.
Record exact source/artifact hashes and which external boundaries remain unverified.
A passing old Python suite or compiling crate alone does not close this block.
