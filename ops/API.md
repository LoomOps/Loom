# Loom API and MCP

The native HTTP/MCP reference is [docs/reference](../docs/reference/README.md). It documents transports, scopes and every registered action with exact input schema, examples and implementation ownership. For a connected installation use `system.describe`; it is filtered by the caller's scopes and matches the running binary.

`ops/job_contract.py`, `mcp_docs.py` and `mcp_groups.py` retain the previous Python implementation for compatibility tests. Native authority is `rust/crates/core/src/contracts.rs`, `http.rs` and `store.rs`. Do not derive current native behavior from the retained schema alone.

Use [task guides](../loom_use/entry.md) for workflows and [implementation boundaries](../docs/IMPLEMENTATION.md) for changes.
