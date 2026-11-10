# Loom Ops

Loom owns Job admission, execution, cancellation, durable state and result indexing.
An external agent submits via API/MCP, reads results and applies its own recovery
policy. Loom does not require a particular agent, chat/thread layout or callback scheduler.

## Development and deployment layout

The username and DNS names below are placeholders; use your installation configuration.

- Source: `/home/developer/codex/loom`.
- Development: `/loom/ops/dev-build`, `/storage/loom/dev-build`; use `bash deploy/compose.sh`.
- Alpha instance: `/loom/ops`, `/storage/loom/data`; use `bash deploy/runtime.sh` only for authorized releases.
- Both run `loom-ops` (API + worker) and `loom-dashboard`; alpha also runs a separate `loom-gateway`. DBs, credentials and queues are separate.
- Public Gateway: `https://loom-machine.example.ts.net/` (443), request/redirect only.
- Alpha Dashboard/MCP: `https://loom-machine.example.ts.net:8443/`, authenticated.
- Dev Dashboard: `https://loom-machine.example.ts.net:18443/`, explicitly tokenless.
- Only private Dashboard origins expose `/mcp`. Dev publication is optional and exists only when explicitly configured; no public Gateway MCP.
- Numbered build snapshots and image IDs are in `/loom/builds`. Do not overwrite a numbered image.
- Dev UI has live source mounts; Ops changes require a restart once its active Jobs return.
- External sources/build recipes can live under `/dependencies`; runtime images and grants are selected through profiles and the host executor.
- The running alpha image remains frozen. Source and dev changes are not automatic alpha releases.

## Shared execution

See [the current platform contract](../deploy/PLATFORM.md) for status phases,
recovery reservations, the six MCP groups, user component registration, runtime
isolation and installation. Domain checks belong to user components and profiles.

## API and MCP

`GET /` returns API discovery JSON; the web UI is served by loom-dashboard.
`GET /api/status` exposes machine, storage, permissions, build and release-check status.
`POST /api/jobs` accepts `key`, `kind`, `owner`, `notify` and `params`.
Read the selected profile's actual parameter schema through `components.profiles`;
research profiles are registered extensions, not core installation requirements.
`GET /api/jobs/{id}` returns durable state, the Attempt and typed result.
`POST /api/jobs/cancel` accepts `id`. `GET /api/events` lists the
authenticated recipient's pending errors. `POST /api/events/ack` accepts
`seq` to acknowledge a durable event.

### Job classification contract (`loom-job/v2`)

Submission optionally accepts `category` and `execution_mode`, separate from
the execution-profile selector `kind`. Categories are `sdg`, `data_preparation`,
`fine_tuning`, `evaluation`, `pipeline`, `other`; modes are `full`, `pilot`,
`smoke`, or null. Omitted values mean `other` / null (unrecorded), including old
history. Classification never changes dispatch, parameters, verification or
Task level. The same idempotency key with different classification is a conflict.

`GET /api/jobs/contract` / MCP `system.contract` returns the canonical enums,
submit schema and list schema. `GET /api/jobs` / MCP `jobs.list` accepts optional
`category`, `execution_mode`, `offset` (default 0), `limit` (default 250, max 1000).
Native and imported Jobs are sorted together, filtered, then paginated; the
response remains an array for compatibility. Continue paging until fewer than
`limit` entries are returned. Pages reflect current state, not a frozen export.
Detail, list and dashboard catalog expose the same classification.

`GET /api/jobs/profiles` / MCP `components.profiles` describes permitted
runner IDs, exact input locator schemas, output identities and runtime limits.
It omits executable paths and secrets. A read-only actor without submit scope
gets no executable profiles. Component profiles define their parameter schema and execution limits; selecting
`full` metadata does not enlarge those limits.

The Dashboard shows category and mode in Job properties and queue rows, with
filters shared by current queue and terminal history. It receives enum labels
from the catalog's contract. Unknown historical metadata is not guessed from
Job titles, and original historical records are not rewritten.

Use trusted Tailscale identity or an explicit scoped Bearer token. The stateless
Streamable HTTP endpoint is `POST /mcp`, with initialize, tools/list and tools/call.
Discovery is filtered by scopes. Authenticated users share Job and result access;
ownership is a responsibility property. Action scopes, notification recipients,
runtime grants and recovery leases remain enforced. Identical submissions from the
same submitter with the same key return the same Job. The worker owns durable
execution after the client disconnects. Interrupted work and failures retain their Attempts and enter the bounded agent
recovery flow described in [the current guide](../loom_use/recovery.md). Queue ordering
and waiting holds use the ordinary submit scope; see [Jobs](../loom_use/jobs.md).
Use the grouped recovery contract for repair claims and retries.

The result manifest has `entities` and `metrics`. Supported entity kinds
are artifact, dataset, model and experiment. The worker validates IDs,
output-relative regular files and finite metrics, hashes files and commits
typed records to the job result after child exit. The built-in cpu.smoke profile is a bounded verification workload. Real workloads register reusable components and profiles through MCP/API; no platform-specific research adapter or central service is required.

## History and migration

`history_store.py` is the active immutable history store, not an old dispatcher.
Existing Job state, metric values and verification remain readable.
Imported records are read-only projections in `history_records`; they never enter the executable queue. One-off conversion scripts are not part of the platform. Use [portability](REMOTE-FENCING.md) for a deliberately planned Loom-to-Loom move.

## Validation

Use `bash deploy/test.sh --browser`. Tests use temporary SQLite/file fixtures,
mock infrastructure and actual isolated CPU subprocesses. They do not run GPU
science or write production DBs. See [TESTING.md](../deploy/TESTING.md).

[API](API.md), [operations](../docs/UPDATES.md), [remote ownership](REMOTE-FENCING.md).
