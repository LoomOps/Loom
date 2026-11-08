# Package inventory

| Item | Authority | Update mechanism |
|---|---|---|
| Operational guides | Running Loom image | system.guide and guide bundle |
| MCP/action schemas | Running Loom code | system.describe and tools/list |
| Job schema | Running Loom code | job-contract.json / API |
| Client release declaration | Loom feature review and contract CI | client_release in guide manifest |
| Client installer, authentication and connections | LoomOps/Loom-Use | Exact pinned public release |
| User Skill | User-installed Loom Use | Client install/update |
| Legacy Skill bridge | Server guide snapshot | Directs existing links to independent client |
| Tokens and connection profiles | Current OS user | Private files; never distributed |

Release pins do not alter Jobs, data, host dependencies or agent recovery policies.
