---
name: loom-use
description: Use an existing Loom connection and migrate to the independent Loom Use client.
---

# Loom Use

This is the compatibility entry for an existing server-installed Skill link.
The client now lives at https://github.com/LoomOps/Loom-Use.
For an existing MCP connection, read system.status and system.guide, then use
system.describe for action schemas. Preserve current Jobs and credentials.
For client installation, connection changes or updates, obtain the client_release
specified by system.guide and read its INSTALL.md. Do not assume this server guide
snapshot contains client scripts. Configure the current user, never the Loom account.
Do not infer current-chat readiness from a standalone transport probe.
