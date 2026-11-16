---
name: loom-use
description: Compatibility entry for existing Loom server-guide Skill links. Read the connected server's task guides and use the independently released Loom Use client for local setup.
---

# Loom guide bridge

If MCP is connected, call `system.status`, `system.guide`, and load the relevant task guide. Use `system.describe({group,action})` for exact arguments. The running binary defines supported behavior.

This server bundle contains guides, not the client installer. For requested connection setup or repair, use the exact independent Loom-Use release declared by `client_release` in the guide manifest. Its checksum-verified package contains `INSTALL.md` and the current client Skills. Do not treat this directory as the client root or install server software as a connection step.
