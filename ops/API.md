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
| /api/registry | results.list | Native artifacts plus active-catalog historical artifacts; historical entries include history_ref for results.historical |
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
execution_mode, depends_on, execution, output_storage and queue_position (tail/next). Use
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

The Dashboard's Extensions label does not rename the `components` tool group.
Its list_tools/get_tool/register_tool actions manage independent research Tools;
list/get/write/register manage Job lifecycle Components, and Profile actions pin
execution contracts. Old Job/Profile pins and historical names remain preserved.

## Logical Job and internal steps

One user-requested generation, training or evaluation outcome is one Job by
default. Batches, shards, cases and scheduling slices are internal implementation
units, not reasons to submit separate Jobs. Use prepare/run/finalize and a pinned
runner that loops, parallelizes or vectorizes within its allocated resources.
Acceleration must preserve the scientific contract; validate equivalence before
changing vectorization, clocks or resource assumptions. Independent requested
outcomes, or explicitly approved distributed execution, can use separate Jobs.
Do not silently turn output/time budgets into a chain of scheduler Jobs. Set a
verified whole-Job budget or report the limitation before submission.

A Profile may declare `steps: [{"id":"generate","label":"Generate cases"}]`.
The request's `steps` descriptor identifies the plan and `steps.json` report file.
The producer atomically replaces that file in request.output_dir with
`{"schema":"loom-job-steps/v1","profile_sha256":"<request.profile.sha256>",
"steps":[{"id":"generate","state":"running","detail":"12 / 140"}]}`.
Allowed states are pending/running/completed/reused/failed/skipped. IDs/labels
come from the pinned plan; the file is bounded to 256 KiB and 256 steps. Step
progress is producer-reported display evidence, never a resource-return,
execution-success or artifact-validation gate. A failed or unreadable report
must not change Job execution state.

## Consolidate earlier fragmented execution

Existing Job/Attempt history and output provenance are immutable evidence. Do
not delete rows or relabel past executions as if one process produced them.
With explicit user authorization, submit one continuation Job using
`consolidates: ["<completed-source-job>", ...]`, pinned inputs and a stable key.
Sources must be completed with verified outputs and have the same Owner. They
become dependencies; the runner verifies/reuses exact bytes and generates only
missing work. Reused steps name their frozen source_job/source_attempt. No
scientific condition may run twice merely to simplify the presentation.

Only after the canonical Job completes with verified artifacts does the default
Job list show it in place of its constituent Jobs. `jobs.list` with
`include_consolidated:true`, direct `jobs.get` and the Dashboard's source-history
filter retain access to every original. Job details expose consolidation sources
and internal progress. A pending or failed canonical Job does not hide sources;
repair it with the same key/Job instead of making competing continuations.
Dataset manifests can reference earlier registered archives; consolidation does
not imply copying every file or a new scientific generation.
