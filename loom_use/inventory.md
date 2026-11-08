# Loom Use package inventory

| Item | Location or interface | Updated by |
|---|---|---|
| Server entry guide | entry.md / system.guide(id=entry) | Running Ops image |
| First use | quickstart.md | Running Ops image |
| Job states and pipelines | jobs.md / system.contract | Guides and execution contract |
| Components, profiles, dependencies | components.md / components.profiles | Guides and registered user revisions |
| Errors, completion, recovery | recovery.md / events, recovery | Guides and actual Job records |
| Version updates | updates.md / manifest.json | Server build, contract and guide identifiers |
| Server layout and administration | operator.md, deployment source deploy/*.py and Compose | Platform administrator |
| MCP tool list | tools/list / bundle mcp-tools.json | Server schema filtered by current scopes |
| MCP and HTTP action reference | system.describe / bundle mcp-reference.json | Actual action schemas |
| Job input and recovery contract | system.contract / bundle job-contract.json | Actual server contract |
| Download helper | refresh.py | Python 3 standard library only |
| Optional stdio adapter | clients/stdio.py / clients/README.md | Clients without direct HTTP MCP support |
| Optional agent skill | skills/loom-use/SKILL.md | Minimal entry instructions linking to server guides |
| Dashboard | Loom URL / Explorer, Models, Datasets, Experiments, Queue, Docker, MCP | Separate Dashboard image |
| External execution environments | /dependencies and pinned image digests | Administrator or component maintainer |

Bundles exclude tokens, databases, datasets, models, runtime binaries and other users' profile source. MCP reference material is filtered by the downloading actor's scopes. Query the connected server for available profiles and Jobs rather than treating a bundle as a live execution inventory.

Server-specific entry skills, SSH addresses and personal path instructions are not shared package contents. Existing server skills remain independent; the common package supplies only `loom-use`. Historical operating documents and personal agent role hierarchies are excluded.
