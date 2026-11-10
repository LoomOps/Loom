# execution — Job lifecycle, script execution and container ownership

## Responsibilities and boundaries

Loom executes Jobs. Release resources based on executor-ticket, container and process return evidence. Never pass platform secrets or Docker sockets into Jobs.

## Inputs, outputs and authoritative sources

- [ops/execution.py](../../ops/execution.py)
- [ops/runtime_contract.py](../../ops/runtime_contract.py)
- [executor/runtime_policy.py](../../executor/runtime_policy.py)

The linked code/schemas define fields, enums and state transitions. This document does not duplicate them.

## Call contracts

| Boundary | Input | Result or failure | Consumers |
|---|---|---|---|
| `ops/execution.py#request_file` | Pinned profile, Job params, stage, input locators | Resolved execution request file; invalid paths/inputs rejected before runtime | api |
| `ops/ops.py#worker` | Durable queue/config/resource receipts | Preparation/run/return/result transitions; no release without resource evidence | queue, recovery, results |

## Related features and change impact

[extensions](extensions.md)

When execution contracts change, verify the worker, executor, restart/idempotency behavior, cancellation, result collection and Compose mounts.

## Tests and failure boundaries

- [tests/ops/test_executor_boundary.py](../../tests/ops/test_executor_boundary.py)

Test sources define the concrete scenarios. `deploy/test.sh` records individual results as JSON/JUnit.

## Live validation

Actual Docker/GPU runtimes and power loss after container creation are outside mock coverage.

## Registered RW storage

Optional runner.storage is a unique list of data/N IDs. Profile registration checks the installation grant; executor startup rechecks IDs, the configured host roots and symlink changes. Paths are never supplied by the caller. Explicit mounts are RW at /loom/data/N and appear in request.storage. Omission preserves existing Job isolation, /work result publication and read-only pinned input mounts. system.status.data_stores reports this capability; both HTTP and MCP use the same profile schema. New installation grants all selected stores, but image updates preserve existing grants and data locations.

## Registered output storage

`ops/storage.py` owns named stores and persistent `.loom-storage.json` identities. `jobs.submit.output_storage` defaults to data/0 and freezes a non-default identity; all Attempt stages and result retries use that root. Result locators carry the selected ID and store-relative path. Registered secondary outputs are available in Dashboard files/media, results API and read-only downstream input mounts. Unknown, redirected, missing or replaced stores must never fall back to the primary. Existing Jobs without this field retain primary behavior; no DB rewrite or data move is required. Profile `storage` mounts remain separate optional shared RW access.

## Administrator GPU/network policy and runtime acceptance

Driver capabilities and network are selected only in protected executor runtime
configuration. Defaults stay compute/utility and none; explicit graphics and
bridge egress do not relax UID, rootfs, mounts or capability isolation. Image
Profiles use the instance declared_runtime policy; compatibility runtime IDs use
their named policy. See [runtime doctor](../RUNTIME-DOCTOR.md) for the opt-in
bounded probe and the distinction between registration and actual acceptance.

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

Consolidated Job reads expose `work_timeline`, derived from the frozen source
Attempt receipts and the continuation's own Attempts. Dashboard Job start/end
cover that entire elapsed period; execution intervals retain the individual
receipts so gaps are not counted as active runtime. Reused steps include
server-derived `source_execution` start/end and Job/Attempt identity. The
continuation's `attempts` and timestamps remain unchanged: they describe the
verification/index run, not the original generation. Missing receipts are
reported and never replaced with invented timestamps. This is a read projection;
no source, runtime timestamp, profile or result manifest is rewritten.
