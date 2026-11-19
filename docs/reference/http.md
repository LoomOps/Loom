# HTTP routes

The unified `POST /api/actions` surface covers every action in the group reference. These convenience routes are extracted from the native HTTP dispatcher. GET uses query arguments; POST uses the action argument object as JSON. The convenience query parser converts offset/limit/after/revision to integers and include_consolidated to a boolean; other query values remain strings. Use /api/actions with typed JSON for arrays, objects and other boolean selectors. Identity-bearing path parameters are URL-decoded. See the exact action schema for accepted fields and scopes.

| Method | Path | Action |
|---|---|---|
| `POST` | `/api/logs/streams` | [`jobs.log_streams`](jobs.md#log_streams) |
| `POST` | `/api/logs/read` | [`jobs.read_log`](jobs.md#read_log) |
| `POST` | `/api/logs/search` | [`jobs.search_logs`](jobs.md#search_logs) |
| `GET` | `/api/dashboard/summary` | [`system.dashboard_summary`](system.md#dashboard_summary) |
| `POST` | `/api/items/search` | [`results.search_items`](results.md#search_items) |
| `POST` | `/api/items/resolve` | [`results.resolve_item`](results.md#resolve_item) |
| `GET` | `/api/search/status` | [`system.search_status`](system.md#search_status) |
| `GET` | `/api/status` | [`system.status`](system.md#status) |
| `GET` | `/api/jobs/contract` | [`system.contract`](system.md#contract) |
| `GET` | `/api/jobs/queue` | [`jobs.queue`](jobs.md#queue) |
| `GET` | `/api/jobs/profiles` | [`components.profiles`](components.md#profiles) |
| `GET` | `/api/jobs` | [`jobs.list`](jobs.md#list) |
| `POST` | `/api/jobs` | [`jobs.submit`](jobs.md#submit) |
| `POST` | `/api/jobs/workflow` | [`jobs.submit_workflow`](jobs.md#submit_workflow) |
| `POST` | `/api/jobs/cancel` | [`jobs.cancel`](jobs.md#cancel) |
| `POST` | `/api/research/curate` | [`jobs.curate`](jobs.md#curate) |
| `GET` | `/api/events` | [`events.list`](events.md#list) |
| `POST` | `/api/events/ack` | [`events.ack`](events.md#ack) |
| `GET` | `/api/registry` | [`results.list`](results.md#list) |
| `GET` | `/api/runtimes` | [`components.runtimes`](components.md#runtimes) |
| `GET` | `/api/docker` | [`system.docker`](system.md#docker) |
| `GET` | `/api/ssh` | [`system.ssh`](system.md#ssh) |
| `GET` | `/api/updates` | [`system.updates`](system.md#updates) |
| `POST` | `/api/updates/check` | [`system.check_updates`](system.md#check_updates) |
| `POST` | `/api/history/list` | [`results.history`](results.md#history) |
| `POST` | `/api/history/get` | [`results.historical`](results.md#historical) |

## Special routes

| Method | Path | Boundary |
|---|---|---|
| GET | `/` | Unauthenticated service discovery only |
| POST | `/mcp` | JSON-RPC initialize, tools/list, tools/call and protocol notifications; operation data is in the first text block |
| POST | `/api/actions` | Unified group/action/arguments dispatcher |
| GET | `/api/principal` | Authenticated principal/scopes, no credential value |
| GET | `/api/guide`, `/api/guide/{id}`, `/api/guide/bundle` | Current manifest/index, one guide or scope-filtered ZIP |
| GET | `/api/jobs/{id}` | jobs.get, excluding contract/queue/profiles reserved paths |
| GET | `/api/registry/{id}` | results.get; optional numeric revision, default 1 |
| GET | `/api/items/{id}`, `/api/items/{id}/relations` | Exact Item or paged relations; IDs remain decimal strings |
| POST | `/api/items/{id}/files` | Paged registered inventory; path supplies item_id |
| GET / POST | `/api/files/{id}` / `/api/files/{id}/read` | Exact file metadata / bounded verified byte range |
| GET | `/api/dashboard/shell`, `/api/dashboard/bootstrap`, `/api/dashboard/catalog`, `/api/dashboard/revision`, `/api/dashboard/resources` | Browser projections and compatibility catalog; shell returns workspace metadata; visible panels request queue occupants, Activity and paginated records independently |
| POST | `/api/events/delivery` | Scoped delivery receipt; separate from recipient ACK and repair lease |
| GET / POST | `/api/admin/tokens` | Signed Dashboard Admin claim; list metadata / issue one-time token |
| POST | `/api/admin/tokens/revoke` | Signed Dashboard Admin claim; revoke by exact token ID |

These are Ops routes, implemented by [http.rs](../../rust/crates/core/src/http.rs) and the service transport. Dashboard is a separate authenticated relay with browser session/CSRF requirements and a restricted set of writes; a Dashboard origin is not an arbitrary Ops proxy. Public Gateway has no operational API. See [Dashboard boundaries](../../dashboard/README.md) and [access enrollment](../ACCESS-ENROLLMENT.md).

MCP is stateless POST. Supported negotiated protocol versions are 2024-11-05, 2025-03-26 and 2025-06-18. An Accept header allowing only text/event-stream receives one message event; this is not a background notification subscription. Notifications return HTTP 202 without an operation reply. Protocol errors and action errors are distinct. Authentication/scope, invalid-input, conflict and unavailable-source failures retain their HTTP/error envelope; a transport success is not Job completion.
