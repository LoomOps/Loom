# ssh — Optional personal host SSH accounts and connection discovery

## Boundary

Admin-only mutations; never adopt existing accounts or grant sudo/Docker. Disable blocks the personal UID before remote cleanup. Same-host clients require no SSH.

The current native service is implemented in `rust/`. Python files in this block may be retained web/administrator integrations or compatibility references; they do not supersede the native transactional handlers. See [implementation boundaries](../IMPLEMENTATION.md).

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Host account authority | `deploy/ssh_host.py#Accounts` | Restricted owner/action/username over protected Unix socket | Managed personal account state or failure; no arbitrary command execution | dashboard, installation |
| SSH connection | `ops/ssh_info.py#connection` | Authenticated principal | Only that principal connection metadata | api, onboarding |

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

Run the affected suites and `deploy/features.py check`; see [the verification matrix](VERIFICATION.md). Passing retained compatibility fixtures is distinct from exercising the native handler.

Acceptance outside these fixtures:
- Verify external personal Tailscale login and revoke separately; mocks do not prove provider enforcement.
