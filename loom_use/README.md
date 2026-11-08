# Loom Use server guides

This directory owns the operational guides shipped by the running Loom server.
The independent client, Skill and installation tools are maintained in
[LoomOps/Loom-Use](https://github.com/LoomOps/Loom-Use).

Start with `system.status`, `system.guide`, then `system.describe` for current action
schemas. There is one MCP endpoint with six groups: jobs, recovery, events,
components, results and system. See entry.md, quickstart.md, jobs.md, components.md,
recovery.md, updates.md and operator.md.

`client-release.json` pins the exact independently tested client artifact and checksum.
The guide manifest exposes it as `client_release`. Server CI uses this release for
real HTTP contract tests. A server code change does not imply a client upgrade;
change the pin when the reviewed contract/client implementation requires it.

`/api/guide/bundle` includes these guides, the client release descriptor and generated
manifest.json, mcp-tools.json, mcp-reference.json and job-contract.json. It does not
contain the client implementation or credentials. `/loom/loom_use/current` retains
server guide snapshots. Its tiny Skill bridge preserves existing installations;
new Skill links target independent user-installed client releases.

License: GPL-3.0-only; see LICENSE.
