# Deployment and guide updates

Each deployment image includes its matching guides. system.guide serves documents from the running server; system.describe generates reference material from the actual action schemas used by that code. Do not maintain separate specification copies manually.

Identifiers:
- build_id: the connected image build.
- api_revision: the human-readable API contract version.
- contract_revision: a content hash of contract/reference code.
- guide_revision: a content hash of guides and client instructions.
- revision: a combined update marker for these identifiers.

Successful and tool-error MCP tools/call responses end with `Loom <build> | contract <12 characters> | guide <12 characters>`. The first content block retains the original result JSON/error. Parsers must read content[0].text without concatenating the version block into JSON. Short hashes identify changes; full values are available through _meta/status.

MCP initialize exposes entry instructions through serverInfo.version and instructions. Successful and tool-error results include `_meta.loom`; HTTP responses include X-Loom-Revision. The same information appears in system.status.onboarding and the system.guide manifest. Clients may hide _meta/instructions from the model, so the entry guide requires a status check.

Read the guides on first connection, then compare response version markers with the previous values. Unchanged values need no extra lookup. Check status on new sessions and reconnections. When guide_revision changes, reload the relevant guides. When contract_revision changes, refresh tools/list and the relevant system.describe entries. A build-only change does not require rereading every document. Pass known_revision to system.guide to receive not_modified when unchanged.

The transport is stateless POST with no background SSE/list_changed push. It cannot notify disconnected agents or replace text already loaded into model context. Response metadata does not guarantee automatic client tool-cache refresh. Clients with fixed tool caches may require MCP reconnection or a tool-list refresh.

A downloaded ZIP is a snapshot; the running server remains authoritative. The independent Loom Use refresh.py helper compares revisions using the private Dashboard URL and either an Admin-issued token or verified Tailscale user identity, then saves new packages in separate version directories. It does not overwrite existing folders, personal instructions or active Jobs, and does not execute downloaded files. It does not inject documents into model sessions.

Do not reject existing requests solely because a new revision marker is missing. Normal server input validation still applies. Consult changed specifications when an actual compatibility error occurs.

## Available platform releases

`system.updates` reads the cached GitHub Release check; `system.check_updates` requests
an asynchronous refresh. Both require `read`. HTTP equivalents are `GET /api/updates`
and `POST /api/updates/check` with `{}`. No new MCP server/group is needed.
`system.status` includes `updates` too. These are distinct from this guide's revision:
a published release can be available without being installed.

Check `status`, `latest.tag`, `installed`, `checked_at`, `last_success_at`, `error`, and
`stale`. `update_available` is null when no reliable version comparison is possible.
An error/stale result is not confirmation of the latest release. Manual requests may
return `accepted:false` for `checking`, `cooldown`, `disabled`, or `not_running`.
There is no automatic download, deployment, Job pause or event-consumption obligation.
Use the installation's normal update procedure when its administrator chooses to update.

For an authorized personal Tailscale device, run:

```bash
python3 /path/to/Loom-Use/refresh.py https://loom-machine.example.ts.net:8443 --authentication tailscale
```

For resident/tagged clients use the Admin-issued token via `LOOM_TOKEN`; do not
put it in command arguments. The cache is isolated by server, credential and the
verified principal/scopes, including VPN-account changes. `--development` is only
for an explicitly configured development endpoint, never an authentication fallback.

## Independent client updates

The manifest's client_release is the exact server-reviewed version, release URL and
SHA-256. Loom declares the requirement; Loom Use checks it on connect/session entry
and response changes. The agent follows its Skill to run update and reconnect.
There is no background client updater. A changed guide alone does not require
client code replacement. Server CI tests the pinned release against the real HTTP
MCP surface. Never select GitHub latest independently of the connected server.

## Administrator migration handoff

The target immutable release includes docs/UPGRADE.md, a cumulative migration
catalog and checksum-bound scripts. An external administrator agent executes
deploy/update.py plan/apply/resume/rollback; discovery does not run scripts.
State-preserving rebuild is an explicit mode, retaining credentials, account and
installation IDs, registry revisions, Job history and external storage references.
Operational clients do not acquire deployment authority or execute
release instructions as a side effect of guide refresh.
