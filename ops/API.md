# Loom API and MCP

The running server is authoritative: `system.describe` returns the visible action
schemas/examples; `system.contract` returns Job status, submission and queue/recovery
policies. The source API revision is defined in `ops/job_contract.py`. MCP `initialize.serverInfo.version`
identifies the deployed build, which may differ from the working source.

Six MCP tools group actions: jobs, recovery, events, components, results, system.
POST `/mcp` accepts initialize, tools/list and tools/call. POST `/api/actions`
accepts `{group, action, arguments}`. Both share execution and scope checks.
MCP success JSON remains in the first text content; the final text is a concise
build/contract/guide identifier, and `_meta.loom` contains full revisions.
HTTP returns the operation JSON. Compare revisions on reconnect or a response;
refresh only changed guide/contract content, without a background polling gate.

| Read endpoint | Grouped action | Meaning |
|---|---|---|
| /api/status | system.status | Machine/build/permissions/admission |
| /api/jobs | jobs.list | All shared Jobs and permitted historical records, newest first |
| /api/jobs/queue | jobs.queue | Shared minimal execution order and waiting reasons |
| /api/jobs/{id} | jobs.get | Authorized Job/Attempts/scheduling/recovery/change history |
| /api/jobs/contract | system.contract | Job/schema/queue/recovery policy |
| /api/jobs/profiles | components.profiles | Callable profiles under caller grant |
| /api/registry | results.list | Authorized artifacts/locations |
| /api/registry/{id} | results.get | Revision 1 over this compatibility route |
| /api/events | events.list | Recipient-scoped pending errors; use kind=all for completion |
| /api/docker | system.docker | Scoped cached host/container observations |
| /api/guide | system.guide | Current onboarding manifest |
| /api/updates | system.updates | Cached published-release check; no automatic install |

For an exact entity revision use results.get with revision. Job list filters are
category/execution_mode/offset/limit; queue order is a separate view. Read scope
allows shared Job parameters, logs, artifacts and queue state. submit scope permits
reordering, holding and resuming shared waiting Jobs, including another user's Job.
Ownership denotes responsibility, not an access restriction. Runtime grants,
recipient-scoped events and recovery leases remain enforced. cancel is a separate scope.

Use jobs.submit with key/kind/owner/notify/params and optional category,
execution_mode, depends_on, execution and queue_position (tail/next). Use
jobs.reorder with id/position (next/tail/before/after), reference only for relative
positions, and optional reason. jobs.hold and jobs.resume take id and optional
reason. Running Jobs return changed=false; they are never checkpoint-paused by
these actions. Normal resume does not repair failures. See [Jobs](../loom_use/jobs.md)
and [Recovery](../loom_use/recovery.md) for dependencies, preparation and error handling.

Public phases are queued/preparing/ready/running/finalizing/completed with
failed/finalizing_failed/cancelled. `state` retains compatibility values such as
processing/complete/blocked. `scheduling` explains waiting separately. Artifact
validation can unblock consumers before indexing finishes. Reindex/finalization
recovery does not rerun training. Events acknowledge delivery, not recovery.

The workspace iframe receives no credentials. The Dashboard parent uses a local
SSO session or page-memory bearer and an explicit same-origin relay. Dashboard has
no Ops database mount; configured SSO/Sharing have their own private state mounts. Its queue write route accepts
only reorder/hold/resume. Other mutations use the Ops endpoint. Fenced/readonly
admission blocks mutations; failed auth/access returns HTTP 403. The development
noauth flag is explicit and never a release default. MCP operation errors return
result.isError; transport auth errors remain HTTP failures.

MCP accepts only the advertised grouped tool names; explicit HTTP routes retain their documented contracts;
they are not advertised in discovery. Imported history is immutable evidence and
never enters the executable queue. See [platform contract](../deploy/PLATFORM.md),
[metadata](../loom_use/components.md), and [testing](../deploy/TESTING.md).
