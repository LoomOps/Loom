# Loom shared execution

Loom owns execution and resource return. Users and agents own scripts, parameters
and diagnosis. `Job Profile` supplies reusable defaults. Component revisions are
immutable execution snapshots; `/loom/components/<owner>/<id>/src` is editable.
The platform itself has no Sim, Cyclo or model implementation dependency.

## Status and scheduling

Public `status`: queued → preparing → ready → running → finalizing → completed.
Errors use failed/finalizing_failed, cancellation uses cancelled. Compatibility
`state` keeps old complete/processing/result_failed/blocked values for existing
clients and historical records. `status` is the user-facing phase; `scheduling`
and `recovery` explain why a Job is waiting. Deferred dependencies are not deleted.

CPU preparation overlaps GPU execution. A prepare runner may declare
`depends_on: []` and `inputs: []` to prepare independently of future artifacts.
Without these overrides, dependency and input requirements remain those of the
Job. GPU execution releases before finalization and callbacks. Pending result
indexing does not block a consumer once its required artifact has been validated.

Failures reserve their queue position for 900 seconds against later same-lane execution. A user can place an independent waiting Job ahead of this reservation; held Jobs do not reserve a lane. Event acknowledgment
does not claim recovery. A recovery claim grants one agent a lease: 900 seconds
without progress, at most 1800 seconds for that repair round. Resubmission retains
position. Each repair execution, including an opt-in automatic retry, consumes one of
three shared recovery rounds after the initial execution. The automatic
max_attempts limit may reduce retries; it never expands that shared budget. Late claims fail. Deferred Jobs must be reactivated,
which gives them a new queue order; running Jobs are never preempted. Timers,
claims and rounds survive process restarts. Failed dependency chains are skipped
while independent eligible work proceeds. Only the affected execution lane waits;
preparation and other lanes may continue. If the process/container has not returned,
the executor keeps the resource unavailable instead of overlapping owners.

## MCP and HTTP

Only six tools are advertised: jobs, recovery, events, components, results, system.
Each takes `action` and optional `arguments`. `system` action `describe` with
`{group,action}` returns the action schema, scope, response notes and examples.
With empty arguments it returns the complete reference filtered by current scopes.
Dashboard **MCP Reference** uses this same response, with grouped navigation,
input tables, nested JSON Schema, response/error notes and copyable MCP/HTTP examples.
Examples contain placeholders, never live credentials, and are not executed by the UI.
The dashboard's `/api/mcp/docs` is a fixed read-only documentation relay. MCP calls accept only the six advertised groups. Explicit HTTP routes remain available to their documented consumers. Grouped HTTP: POST `/api/actions`
with `{group,action,arguments}`. Completion events use `events.list(kind="all")`;
the default filter is errors-only. Event `ack` means receipt, not remediation.

- jobs: list, get, submit, cancel, logs, queue, reorder, hold, resume, assign_owner.
- recovery: claim, progress, resume, defer, reactivate.
- components: list, get, write, register, profiles, register_profile, get_profile, list_tools, get_tool, register_tool.
- results: list, get, reindex, annotate, reconnect, history, historical, assign_owner.
- events: list, ack.
- system: status, docker, contract, describe, guide, users, updates, check_updates.

Submission uses key/kind/owner/notify/params, optional depends_on, category,
execution_mode, queue_position (tail/next) and execution policy. Named upstream input `{job,entity}` can be
submitted before the upstream artifact exists. Runtime and parameter contracts
come from `components.profiles`. An agent's owner delegation governs its access.

## Components and hooks

Use `components.write` to save source files, then `components.register` with
`id` and `manifest` (or supply small files directly at registration). Manifest:

```json
{"profiles":{"run":{
  "params_schema":{"type":"object","properties":{"epochs":{"type":"integer","minimum":1}},"required":["epochs"],"additionalProperties":false},
  "inputs":{},"outputs":null,
  "runtime":{"executor":"container","runtime_id":"python","lane":"cpu","command":["python3","/component/run.py"]},
  "limits":{"timeout_seconds":86400,"max_attempts":4}
}}}
```

The same runner shape is accepted for optional prepare, finalize, on_error,
on_complete and on_cancel. Finalization/callbacks use CPU with finite timeouts.
The last argv is a request JSON containing params, output_dir, profile, Job context.
The runner writes `result.json` with entities/metrics. Entity provenance is
`{profile: request.profile.revision, producer_sha256: request.profile.sha256}`.
Outputs may be explicitly named or null for a dynamic set of valid typed entities.
Job Type and Execution Mode are descriptive; execution limits are a separate field.

Callbacks receive a stable event_id: external effects must use it as an idempotency
key. Arbitrary external side effects cannot be guaranteed exactly-once across a
crash. Callback outputs are separate from registered artifact files. Callback
failure emits a warning event and retries independently; it never reruns training.
Finalization failure retries finalization only. Agent recovery may replace params
or component profile while retaining all Job revisions and Attempts.

## Execution boundary

Ops does not mount the Docker socket or GPUs. A private host executor starts
Job containers with the approved image, non-root UID, read-only root, dropped
capabilities, per-runtime CPU/memory/PID limits, own output directory and explicit
read-only input/component mounts. No API credentials or other Job directories
are mounted. GPU tickets are exclusive across attached instances. Dev defaults
to CPU-only; alpha may request GPU. Runtime definitions and trusted library/asset
mounts live in the host executor config, never in user component metadata.

The host executor needs local Docker authority. Its credential/socket is accessible
only to Ops; user containers never receive it. Finished containers are collected
after logs are saved, leaving durable tickets and Job history rather than clutter.

## Installation

1. Install Docker Engine/Compose and Python 3 on the host. GPU runtimes additionally
   require the NVIDIA driver and Container Toolkit.
2. Load or build versioned Ops/Dashboard images. Run the installer with sudo: it creates/reuses
   the non-login `loom:loom` account, protected `/loom` parents and the system observation timer. `deploy/install.py --help` creates
   a new authenticated instance, data identity, configuration, loopback ports and
   a local self-signed TLS certificate. It never overwrites an existing instance.
3. Run `sudo python3 deploy/executor-attach.py INSTANCE --instance NAME --data DATA --install-service [--gpu]`. UID/GID are read from the installed instance.
   It creates the private executor grant and installs the host service. The sample unit is
   `executor/loom-executor.service`; adjust its absolute deployment path if needed.
4. Ensure `/loom/components` is writable by the configured Ops UID and start the
   executor, then use `LOOM_INSTANCE=... LOOM_PROJECT=... bash deploy/compose.sh up -d`.
5. Tailscale Serve is the supported external entry. Local certificates are for
   loopback testing; use the existing Tailscale endpoint/certificate for clients.

Backup: stop admissions and let owned executions return, snapshot SQLite with its
backup API, then save configuration, credentials, components, data references and
executor ticket DB. Updates deploy new immutable images; do not overwrite existing
tags. DB additions are additive. Restore the matching configuration and images
from the pre-upgrade archive for rollback. Never start two workers on the same DB.


### Grouped HTTP ingress

Expose `/api/actions` to the **Ops API** alongside `/mcp`, not to the limited
Dashboard relay. Configure ingress with `deploy/tailnet.py serve` using the selected instance's API/Dashboard ports and origin; do not copy another installation's ports.

## Workspace and host layout

User workspaces may live anywhere (for example `~/work/<project>`); Loom does not
require them under `/loom`. Register the code/profile revision and explicitly
connect required input/output storage for execution. A Job container cannot
implicitly read arbitrary host workspace paths.

`/loom/ops` holds instance state/configuration, `/loom/components` holds registered
components and optional runtime metadata, `/loom/loom_use` holds operator guidance,
and `/loom/shared` holds shared host services. An optional handoff authority can be installed at
`/loom/shared/authority`, managed by system unit `loom-authority.service` as `loom:loom`. Its durable claim state must be preserved during relocation.

## Independent profiles and metadata (v28)

A component owns reusable source. A profile owns runtime/lifecycle/parameter and
output contracts. A Job freezes a profile revision plus concrete values.
`components.register` accepts `manifest: {}` for source-only registration.
`components.register_profile` accepts `{id, component?: {id, revision}, profile}`;
`components.get_profile` reads a selected or latest revision. The existing
`components.profiles` lists all callable kinds. Six MCP groups remain unchanged.

Registered profile files live under `/loom/profiles/<owner>/<id>/revisions/N.json`.
Dev uses `/loom/profiles/dev-build` as its host root and `/loom/components/dev-build` for component snapshots. The executor maps the container component path to its instance-specific host root. Fresh installations use per-project component/profile roots. Components and profiles are shared across authenticated users. Registration requires submit scope. Existing embedded
component profiles and frozen historical Jobs remain usable.

The profile accepts params_schema, defaults, inputs, outputs, runtime, optional
prepare/finalize/on_complete/on_error/on_cancel, limits and metadata_schemas.
`metadata_schemas` maps an output ID (or `*`) to a local JSON Schema for that
entity's `metadata` object. Missing required metadata is rejected during result
processing without rerunning the computation. Each indexed entity preserves
metadata, its schema, optional label/description, lineage and metric definitions.
The dashboard receives these same metadata fields. Scientific interpretation is
still the producer's responsibility; Job Type does not choose an output schema.

A runner may use either an installed runtime_id or `image` plus optional
`resources: {cpus, memory, pids}`. Registered image tags resolve to immutable local
Docker image IDs. Images must already be cached on the host: build/pull them as
part of the agent-owned environment preparation, not inside core deployment.
New cached images need no platform build, executor configuration edit or restart.
The host's generic declared_runtime policy caps resources; network defaults to none (administrators may explicitly select bridge egress) and
arbitrary mounts, Docker arguments and privileges are not accepted from profiles.
GPU remains an explicit lane and instance grant. Administrator-owned compatibility
runtimes may retain read-only dataset/library mounts for existing Jobs.

The private executor is installed with root-owned server.py, runtime_policy.py and runtime_store.py beneath protected parents. Compose and Job runner UID/GID are taken from the loom service identity. Fresh
installation creates owned components/profile directories. Attach it with
`deploy/executor-attach.py`; see its help for instance/data/UID/GID options.

## Historical operational retirement

Original queue, catalog, publication and run-history rows are preserved in
history under source `archive.queue`, `archive.catalog`, `archive.publication`,
`archive.run-history`, kind `raw-<table>`; original columns are in body.original.
The private original SQLite snapshots also survive. Auth issuer/credential tables
are excluded from API history. The catalog projection is independent of raw table
records and preserves existing verified file locations. No archived record is
submitted to the live execution queue or assigned a new scientific verification.

Historical records are data, not runnable deployment instructions. No personal agent routing or research-specific runtime is shipped by the platform.

## Versioned onboarding

The deployed Ops image serves `system.guide` and authenticated `/api/guide` /
`/api/guide/bundle`. Read [the guide index](../loom_use/README.md) for entry,
Jobs, components, recovery and update behavior. Six MCP groups remain; `guide` is
an action of `system`. Reconnect checks and response metadata expose revisions;
there is no background push or forced client instruction replacement. Shared
external sources/build recipes live under `/dependencies`; retained runtime mounts
remain pinned and are not silently replaced by a new Cyclo image.

## Shared queue controls

Ordinary submit-scoped users can reorder shared waiting Jobs with `jobs.reorder`:
next/tail/before/after. Relative positions use reference Job ID, including another
user's waiting Job. `jobs.queue` / GET `/api/jobs/queue` returns shared IDs, owners,
phases, ranks, waiting reasons and caller capabilities. `jobs.list` is a shared,
newest-submission-first history listing; parameters, logs and results are also
shared. Ownership records responsibility rather than restricting access. Action
scopes, recipient event boundaries, recovery leases and runtime policy still apply.

`jobs.hold`/`jobs.resume` toggle a persisted dispatch flag, not a new execution
status. Already running preparation finishes; the next run waits. Running Jobs
are never preempted. Dependencies, retry delay, lane ownership and result handling
remain authoritative. Queue controls and claims use the same SQLite write lock;
a racing claim returns changed=false to later control requests. Changes record
actor, optional reason and before/after order. No approval or new audit gate is
added to dispatch. Concurrent accepted changes apply in server order; repeated
front insertion can starve tail work and is deliberately left to user coordination.

The Dashboard's current queue uses this same ordering and eligibility. Its
POST `/api/jobs/queue-actions` relay accepts only reorder/hold/resume, with the same
bearer and Origin checks. Arbitrary writes remain unavailable there. Read-only
users see shared minimal order without controls; stale anchors require refresh.

## Executor create recovery

New containers carry instance/Job/stage/ticket plus hashes of the canonical
request and resolved Docker launch command. If create succeeds but its response
or ticket persistence fails, an identical start request can reuse that exact
container while it is still `created`. It commits the ticket before starting it.
Recorded running/returned tickets keep their original idempotent behavior.
Unrecorded running/exited containers, changed requests/runtime configuration and
foreign or unlabeled name collisions are left untouched and reported as conflicts.
No broad orphan deletion, new queue gate or periodic cleanup service is added.
Old tickets continue working without the new labels; old unregistered containers
without identity evidence require explicit operator inspection.

Driver libraries and network policy are administrator-owned executor settings.
Defaults remain compute/utility and no network; explicit graphics or bridge egress
requires a supported host policy. Neither is an arbitrary Job/Profile Docker flag.
Image/source registration and an old receipt do not prove runtime acceptance.
