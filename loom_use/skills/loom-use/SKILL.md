---
name: loom-use
description: Compatibility entry for existing Loom server-guide Skill links. Read the connected server's task guides and use the independently released Loom Use client for local setup.
---

# Loom guide bridge

Confirm the connection with `system.status`. Load the relevant server guide through `system.guide` and the exact action schema through `system.describe({group,action})`. The `jobs` and `catalog-cases` guides cover execution and publication boundaries.

For requested client setup or repair, use `INSTALL.md` from the checksum-verified Loom Use package declared by `client_release` in the guide manifest.
