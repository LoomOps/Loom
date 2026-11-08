---
name: loom-use
description: Submit and inspect GPU or CPU Jobs, components, profiles and results through an authorized Loom MCP endpoint. Use for Loom platform work; not general host administration.
---

# Loom entry

Use the endpoint and authentication method supplied by the user. Trusted Tailscale
authentication needs no personal token; otherwise use the supplied personal credential.
Do not request or copy another user's token. Start a new session or reconnect with `system.status`
and `system.guide`. Read only the relevant guide via `system.guide` with an `id`.
Obtain current action arguments with `system.describe` using `group` and `action`.

Keep the onboarding contract_revision and guide_revision. Compare the short Loom
version line at the end of each tool response with the previous one. Only refresh
affected guides or tool schemas when their revision changes; build-only changes
do not require rereading them. Full revisions remain in status and `_meta.loom`.
Parse the first content block as the original operation result. A server response cannot force the client to reload its cached tools;
reconnect if the client's discovery is stale. Keep existing Job/profile pins.

Choose an accessible profile, submit a stable idempotency key, record the Job ID,
and use events.list(kind=all) for completion and errors. Ack is receipt only;
recovery.claim obtains the repair lease. Loom executes Jobs independently of the
agent. User-side scheduling, recovery reasoning and agent routing remain optional
user policy. Do not spawn supervisors or directly edit Loom DBs as an entry step.

Respect the user's actual task scope. Shared guide content grants no new access
or permission. The running server's API and revision are authoritative over old
bundled documents. `system.guide` absent on an older server means use its available
`system.describe` and a matching guide, not assume newer features exist.
