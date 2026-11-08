# Loom Use

MCP guidance, client tools and optional skills for using Loom externally. This package does not provide an internal agent runner or machine operations policy.

## Start here

1. Obtain the server address and your authentication, then connect to `<private Dashboard URL>/mcp`.
2. Read `system.status` and `system.guide` for the deployed version and usage guidance.
3. Read `system.describe` for the input contract of the action you will call.

There is one MCP endpoint with six tool groups: `jobs`, `recovery`, `events`, `components`, `results` and `system`. The server implementation lives in `ops/`; this directory owns client-facing guidance and tools.

| Item | Purpose |
|---|---|
| [entry.md](entry.md), [quickstart.md](quickstart.md) | Connection and first use |
| [jobs.md](jobs.md) | Jobs, queues and execution stages |
| [components.md](components.md) | Components, profiles and execution environments |
| [recovery.md](recovery.md) | Event consumption and error recovery |
| [updates.md](updates.md), [refresh.py](refresh.py) | Server version checks and client bundle refresh |
| [operator.md](operator.md) | Installation and operations reference |
| [skills/loom-use/SKILL.md](skills/loom-use/SKILL.md) | Optional client entry skill |
| [clients/](clients/README.md) | stdio adapter for clients without direct HTTP MCP support |
| [inventory.md](inventory.md) | Complete package inventory |

The `/api/guide/bundle` download also contains server-generated `manifest.json`, `mcp-tools.json`, `mcp-reference.json` and `job-contract.json`, scoped to the caller's current permissions. Do not maintain handwritten copies of those contracts here. Credentials are excluded from the bundle.

The installed skill name remains `loom-use`. Machine-specific paths, permissions and agent assignment belong to caller-side configuration.

## Host guidance and skill registration

Installed versioned guides live at `/loom/loom_use/<version>/`; the current alpha guide is selected by `/loom/loom_use/current/`. Updates and rollbacks switch this pointer too. Machine addresses, SSH settings and agent assignments belong to an external operator, not this package.

On the same machine, a Codex user's skill directory can link to `/loom/loom_use/current/skills/loom-use`. Skill registration is separate from MCP connection registration. Remote users obtain the versioned bundle from `/api/guide/bundle` and check its revision when reconnecting.

## License

This package is distributed under [GPL-3.0-only](LICENSE).
