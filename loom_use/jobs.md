# Jobs and pipelines

Public status: queued → preparing → ready → running → finalizing → completed. Failures are failed/finalizing_failed; cancellation is cancelled. scheduling/recovery explain waiting and are not execution stages. Do not confuse these with internal compatibility states such as complete/processing.

Job Properties include Job Type(category): sdg, data_preparation, fine_tuning, evaluation, pipeline or other. Execution Mode is full/pilot/smoke, or null when unrecorded. These labels do not choose execution code; the profile selected by kind defines behavior.

prepare can perform CPU work while waiting for a GPU. Declare profile.prepare depends_on=[] and inputs=[] for preparation that does not need predecessor outputs. Omitting them preserves the Job's original dependency/input constraints. run cannot proceed without required inputs.

run uses an allocated lane; GPU ownership is released after the actual process/container returns. finalize and on_complete/on_error/on_cancel are separate CPU postprocessing. Publication delay or callback failure must not rerun training. A consumer can start when its required predecessor artifact is verified, even if other publication is still in progress.

Connect Jobs through depends_on and `{job,entity}` input references. Skipping a failed predecessor does not allow a dependent Job to run without its output; only independent Jobs proceed. A Job timeout limits execution time. Automatic retries and agent-triggered reruns share a maximum three-round recovery budget; automatic retries also obey max_attempts.

Pin Component/Tool/Profile/Runtime revisions and image digests used by execution. New deployments or guidance changes do not change a running Job's revision. Registering a normal new profile does not require a platform deployment.

## Queue order and holds

jobs.submit queue_position is tail(default) or next. jobs.queue returns the shared queue's IDs, owners, current stages, positions, waiting reasons and available controls. Parameters, logs and artifacts are also shared. jobs.list shows logical Jobs by newest submission first; include_consolidated=true includes preserved constituent executions.

With submit scope, call jobs.reorder on shared Jobs. position is next, tail, before or after; only before/after require reference(Job ID). The reference may be another user's waiting Job. reason is optional. Users coordinate order without operator approval. queue_history records the actor, before/after order and reason. Concurrent changes apply in the order processed by the server.

jobs.hold prevents new stages from starting; jobs.resume releases the hold at the same position. A Job held during preparation finishes preparation, then waits in ready. An already running Job is not interrupted: changed=false, reason=not_waiting. If a stale reference has started, reread the queue and choose another reference. recovery.resume reruns after an error; jobs.resume releases an ordinary hold.

Position is priority among eligible Jobs, not a guaranteed start time or strict serialization of every stage. Jobs blocked by unavailable dependencies, holds, retry delays or occupied lanes are skipped. CPU preparation and GPU execution remain pipelined. An independent Job moved ahead of a failed Job's reservation may run first; dependent Jobs still require successful outputs. Starvation prevention is not automatically enforced: users coordinate using the change history.

Resending the same key returns the existing Job without moving it forward again. Changing queue_position while reusing that key conflicts; use jobs.reorder instead. Restarting or holding preserves order. Completed historical Jobs cannot be reordered or resumed.

An idempotency key identifies the original accepted request, including its frozen
defaults. Replay that original request after recovery; a recovery patch changes
the execution revision, not the key's identity. A different request with the same
key conflicts. Current execution results are separate from prior Attempt results;
an active legacy Job may expose `previous_result` as preserved evidence.

Eligible means the local Ops dependency, hold, reservation and lane conditions are satisfied. At actual container startup, the host executor also checks GPU ownership by other instances; UI eligibility does not guarantee resource acquisition.

## Owner

owner_profile and properties.owner identify the current responsible person, not an access boundary. Discover profiles with system.users and assign with jobs.assign_owner({id,owner}). Original spec.owner, submitter, callback notify and execution snapshots are preserved separately. Results inherit the current Job Owner at publication and can later be reassigned through results.assign_owner.

## Output storage

Read `system.status.data_stores`, then set `output_storage` on `jobs.submit`, for example `"output_storage":"data/1"`. Omission uses data/0. Loom freezes the selected store identity for the Job, creates its `runs/<job>/<attempt>` workspace there and passes `/work` as `request.output_dir` to containers. Preparation, run, finalization, callbacks and logs follow that workspace. The producer keeps writing relative paths in result.json; it must not construct host paths or edit the DB.

Loom validates the files/hashes and indexes the selected storage ID, relative paths, metadata, metrics and lineage. Dependent Jobs can use `{ "job":"<producer-id>", "entity":"<output-id>" }`; registered locator inputs work across stores too. Inputs are mounted read-only, without mounting their entire store. `runtime.storage` is a separate opt-in for general shared RW access, not the output selector.

A cancelled or failed execution never supplies dependency readiness merely
because an old result contains `artifact_ready`. Early consumption is limited to
verified artifacts of successful returned execution in finalizing/finalizing_failed,
or completed Jobs. Reindexing preserves the entire published output identity set,
hashes, metadata and lineage. Changed results are rejected, not silently published
over the immutable Registry. Use reconnect for an identical replacement copy.

A missing/replaced store fails that Job instead of falling back to data/0. Result-indexing retries use the same output store and do not rerun compute. Recovery cannot change output_storage on an existing Job; submit a new Job to choose another output store. Image updates preserve configured paths and markers. Older installations expose data/0 by default; additional output stores require their persistent storage IDs and executor grants to be configured, not just an Explorer mount.

## Reconnect a restored artifact

After restoring the exact bytes of an existing artifact, use `results.reconnect`
with `id`, `revision`, the original `component` reference, the new store-relative
`ref`, and optional `storage` (default data/0). Discover the exact schema first.
Loom checks the registered store, path, immutable hash and size, then records the
location. This neither republishes different contents nor reruns the Job. Readers
cannot reconnect; ordinary users with submit scope can. Use `results.get` to
inspect the registered locations afterwards.

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

`jobs.get.work_timeline` derives the whole elapsed period from frozen source
Attempt receipts plus the continuation run. Reused internal steps include
server-derived `source_execution` timestamps. Dashboard start/end and execution
history include the original runs; `attempts` retain the continuation's actual
runtime timestamps. Do not backdate Attempts or overwrite source records to
correct a consolidated Job's display. Missing receipts remain explicitly unknown.


## Pinned execution graphs and research presentation

When a requested outcome requires multiple Profiles or CPU/GPU stages, use
`jobs.submit_workflow` after reading its live schema. Submit `nodes` with unique
IDs, labels, pinned Profile specs and `depends_on` node IDs. Internal inputs use
`{"node":"producer","entity":"output"}` and must name a declared predecessor.
`outputs` maps public aliases to exact node/entity references. A later consumer
may use `{"job":"<main-id>","entity":"<alias>"}`; submission resolves it to the
original verified producer, preserving lineage and dependency checks.

The complete graph is accepted atomically or rejected without orphan executions.
Replay its exact key/request after uncertainty. Each node uses the ordinary
scheduler, resource-return checks and recovery budget. The main Job has execution
nodes, no invented Attempt. Repair/log/reindex target the actual node; cancel or
hold/resume on the main Job delegates to its nodes. Per-node responses describe
partial control outcomes. Reorder targets a waiting node. A failed node is exposed
in main Job errors; independent eligible nodes may continue. A main completion
requires every node completed; supporting receipts are not new research outcomes.

`presentation` records scope (research/operations), role (main/support/test),
optional label, experiment, request_ref and output_roles. An explicitly requested
research diagnostic can be main; a platform acceptance fixture is operations/test.
Use server-side roles instead of guessing from names, Owner or Built-in origin.
`jobs.list` and `results.list` default to research; `view=all` retains full audit
access and `view=operations` exposes operational checks. Direct reads remain.
The shared queue always retains all real resource consumers.

`jobs.curate` previews additive presentation/binding changes by default. Apply a
reviewed batch with dry_run=false and a stable key/reason. Originals, timestamps,
outputs and execution pins are preserved; applying another batch can restore the
previous overlay. Historical grouping does not reschedule or change dependency
readiness. Prefer an execution graph for new multi-stage work. Platform tests run
in isolated dev instances; production-only acceptance stays operational.

Reusable Profiles can declare presentation.scope/role/output_roles. Submission freezes these defaults; explicit Job presentation overrides them. Platform-check Component tags default new Jobs to operations/test. Neither later tag changes nor display overlays rewrite frozen submission identity.

Declare output_roles for each experiment-shaped output when a Profile produces
multiple experiments (for example an evaluation and a budget receipt). Without
these declarations, ambiguous experiment outputs remain supporting records on
the Job, rather than being counted as independent scientific experiments. Exact
records and input references remain available; a published workflow output alias
explicitly identifies the requested primary result.
