# ssh — Optional personal host SSH accounts and connection discovery

## Boundary

Admin-only mutations; never adopt existing accounts or grant sudo/Docker. Disable blocks the personal UID before remote cleanup. Same-host clients require no SSH.

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Host account authority | `deploy/ssh_host.py#Accounts` | Restricted owner/action/username over protected Unix socket | Managed personal account state or failure; no arbitrary command execution | dashboard, installation |
| SSH connection | `rust/crates/core/src/query.rs#system_action` (`system.ssh`) | Authenticated principal | That principal's connection metadata and the machine access descriptor | api, onboarding |

## Sources and contracts

- [dashboard/ssh_access.py](../../dashboard/ssh_access.py) — integration, packaging or documentation.
- [deploy/ssh_host.py](../../deploy/ssh_host.py) — integration, packaging or documentation.
- [deploy/ssh_setup.py](../../deploy/ssh_setup.py) — integration, packaging or documentation.
- [ops/ssh_info.py](../../ops/ssh_info.py) — retained Python compatibility reference.
- [docs/SSH.md](../../docs/SSH.md) — integration, packaging or documentation.

## Change review

Review host helper, private Admin CSRF routes, Sharing revocation, readonly MCP, installation/update/uninstall and Loom Use together.

Related blocks: [identity](identity.md) · [network](network.md) · [installation](installation.md).

## Verification

- [tests/dashboard/test_ssh_access.py](../../tests/dashboard/test_ssh_access.py)
- [tests/deploy/test_ssh_host.py](../../tests/deploy/test_ssh_host.py)
- [tests/ops/test_ssh_info.py](../../tests/ops/test_ssh_info.py)

Acceptance outside these fixtures:
- Verify external personal Tailscale login and revoke separately; mocks do not prove provider enforcement.

## File handoff consumer

The independent Loom Use client composes existing `results.get_file`,
`system.status` and `system.ssh` responses into an exact File access plan. The
caller keeps the selected connection and File/revision ID. Storage IDs map through
status selectors to `system.ssh.data_paths`; local detection retains these roots.
Another-machine and unsupported copies are explicit results, not guessed paths.
Optional account/file/Python probes run only on request and create no server
receipt, account, key, grant, package or Job. Shared data remains read/write under
the existing installation-specific group; no `ax` membership is introduced.

Server guide authority is `rust/contracts/items.md`; client commands and their
probes are owned by Loom-Use `clients/file_access.py` and `docs/ssh.md`. No new byte
transfer API or host-execution MCP action is introduced. Verify real external
login separately from metadata mapping and synthetic SSH protocol tests.
