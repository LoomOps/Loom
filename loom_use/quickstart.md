# First-use workflow

## Connect and discover

The administrator configures tailnet access and Loom user bindings. Omit tokens only when this client supplies a verified Tailscale user identity. Server-resident Codex and tagged devices use an Admin-issued agent token; see the independent [Loom Use installer](https://github.com/LoomOps/Loom-Use/blob/main/INSTALL.md). Read `system.status` to confirm the destination and admission state, then select an available profile from `components.profiles`. Owner identifies responsibility; users share Job/result visibility and controls. Do not share personal tokens. HTTP callers can inspect their ID/scopes through `GET /api/principal`; MCP callers use `system.status.principal`. Automatically registered Tailscale users use their ID as the default owner/notify and share existing data and profiles.

Inspect the profile's kind, params_schema, inputs, outputs and revision. If none fits, register your own source and profile using the components guide. Replace documentation placeholders with actual profile, owner and notify values before execution.

## Submit

Read the current submission schema through `system.describe` with group=jobs, action=submit.

```json
{"name":"jobs","arguments":{"action":"submit","arguments":{"key":"my-unique-run-001","kind":"<registered-profile-id>","owner":"<your-owner-id>","notify":"<your-agent-id>","params":{},"category":"fine_tuning","execution_mode":"smoke"}}}
```

Save the returned Job ID. If the network response was lost, repeat the same key and input. Use a new key if execution semantics change. Declare budget, data, model and evaluation semantics explicitly in profile/params according to the user's goal.

Use `jobs.get` for stages and Attempts; continue `jobs.logs` using next_offset instead of repeatedly downloading the full log. Read completion/error events with `events.list(kind=all)`. Jobs keep running on the server when the agent disconnects.

After completion, inspect artifacts, locations and provenance through `results.list/get`. Execution completion, result publication and event acknowledgment are distinct facts. First use is not authorization to run a large training experiment as a test.

`results.list` also discovers imported models, datasets and experiment records.
Entries marked `record_type: historical` carry `history_ref`: pass that object to
`results.historical`, not `results.get`. They preserve original metadata and a
separate current-availability observation; discovery does not imply usable weights
or a ready dataset. Native registry entries continue to use `results.get`.

## HTTP alternative

Call the same operations through `POST /api/actions` with `{group,action,arguments}`. Do not forge identity headers when using Tailscale Serve authentication. Token-based callers send `Authorization: Bearer <personal-token>`. Inspect MCP result.isError, or HTTP status and error body, for failures.

Use browser login when Dashboard provides SSO. MCP authenticates separately through the server's Tailscale integration or an issued service credential. Do not copy Dashboard cookies into MCP configuration.
