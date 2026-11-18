# API and MCP reference

Generated from the compiled native action registry and action documentation. Regenerate with `python3 deploy/documentation.py build --binary target/debug/loom-ops`; do not edit generated action pages. The running installation may differ: use `system.describe` there.

## Transport

MCP uses stateless `POST /mcp`: initialize, tools/list and tools/call. A tool call has `params: {"name":"<group>","arguments":{"action":"<action>","arguments":{...}}}`. Read operation JSON from `result.content[0].text`, check `result.isError`, and keep `_meta.loom` separate from the visible version footer. Do not concatenate text blocks into JSON.

HTTP uses `POST /api/actions` with `{"group":"<group>","action":"<action>","arguments":{...}}` and returns the operation JSON directly. Inspect HTTP status and the error body. Both transports use the same application registry, scopes and validation. Use the installation's configured credential or trusted Tailscale ingress; browser cookies and arbitrary identity headers are not credentials.

## Discovery and authorization

`system.status` identifies the target and principal. `system.guide` returns task routes and version markers. `system.describe({group,action})` returns one action; group-only returns the group reference and empty arguments return the full scope-filtered reference. A documented action may be absent for a caller without its scope.

Read scope includes discovery; submit permits the documented mutation/maintenance actions; cancel and events are separate scopes. A read-scoped update check can request background release metadata, not installation. Shared Owner attribution is not access control. Exact Runtime grants, event recipients, recovery leases, admission and filesystem availability impose additional handler preconditions. Scope alone does not guarantee success.

## Action groups

| Group | Actions | Reference |
|---|---:|---|
| `jobs` | 20 | [Job execution and shared queue](jobs.md) |
| `recovery` | 5 | [Bounded incident repair](recovery.md) |
| `events` | 2 | [Recipient lifecycle events](events.md) |
| `components` | 23 | [Profiles, Components, Tools and Runtimes](components.md) |
| `results` | 47 | [Results and versioned Scenario context](results.md) |
| `system` | 18 | [Installation status, contracts and guides](system.md) |

See [concepts](../CONCEPTS.md) for object identity/state distinctions, [task guides](../../loom_use/entry.md) for workflows and [implementation boundaries](../IMPLEMENTATION.md) for maintainers. Input schema validation does not prove successful execution, byte availability or scientific semantics.

See the [HTTP route map](http.md) for convenience and administrator integration endpoints.
