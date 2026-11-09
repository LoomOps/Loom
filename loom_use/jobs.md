# Jobs and pipelines

Public status: queued → preparing → ready → running → finalizing → completed. Failures are failed/finalizing_failed; cancellation is cancelled. scheduling/recovery explain waiting and are not execution stages. Do not confuse these with internal compatibility states such as complete/processing.

Job Properties include Job Type(category): sdg, data_preparation, fine_tuning, evaluation, pipeline or other. Execution Mode is full/pilot/smoke, or null when unrecorded. These labels do not choose execution code; the profile selected by kind defines behavior.

prepare can perform CPU work while waiting for a GPU. Declare profile.prepare depends_on=[] and inputs=[] for preparation that does not need predecessor outputs. Omitting them preserves the Job's original dependency/input constraints. run cannot proceed without required inputs.

run uses an allocated lane; GPU ownership is released after the actual process/container returns. finalize and on_complete/on_error/on_cancel are separate CPU postprocessing. Publication delay or callback failure must not rerun training. A consumer can start when its required predecessor artifact is verified, even if other publication is still in progress.

Connect Jobs through depends_on and `{job,entity}` input references. Skipping a failed predecessor does not allow a dependent Job to run without its output; only independent Jobs proceed. A Job timeout limits execution time. Automatic retries and agent-triggered reruns share a maximum three-round recovery budget; automatic retries also obey max_attempts.

Pin Component/Tool/Profile revisions and image digests used by execution. New deployments or guidance changes do not change a running Job's revision. Registering a normal new profile does not require a platform deployment.

## Queue order and holds

jobs.submit queue_position is tail(default) or next. jobs.queue returns the shared queue's IDs, owners, current stages, positions, waiting reasons and available controls. Parameters, logs and artifacts are also shared. jobs.list shows all Jobs by newest submission first.

With submit scope, call jobs.reorder on shared Jobs. position is next, tail, before or after; only before/after require reference(Job ID). The reference may be another user's waiting Job. reason is optional. Users coordinate order without operator approval. queue_history records the actor, before/after order and reason. Concurrent changes apply in the order processed by the server.

jobs.hold prevents new stages from starting; jobs.resume releases the hold at the same position. A Job held during preparation finishes preparation, then waits in ready. An already running Job is not interrupted: changed=false, reason=not_waiting. If a stale reference has started, reread the queue and choose another reference. recovery.resume reruns after an error; jobs.resume releases an ordinary hold.

Position is priority among eligible Jobs, not a guaranteed start time or strict serialization of every stage. Jobs blocked by unavailable dependencies, holds, retry delays or occupied lanes are skipped. CPU preparation and GPU execution remain pipelined. An independent Job moved ahead of a failed Job's reservation may run first; dependent Jobs still require successful outputs. Starvation prevention is not automatically enforced: users coordinate using the change history.

Resending the same key returns the existing Job without moving it forward again. Changing queue_position while reusing that key conflicts; use jobs.reorder instead. Restarting or holding preserves order. Completed historical Jobs cannot be reordered or resumed.

Eligible means the local Ops dependency, hold, reservation and lane conditions are satisfied. At actual container startup, the host executor also checks GPU ownership by other instances; UI eligibility does not guarantee resource acquisition.

## Owner

owner_profile and properties.owner identify the current responsible person, not an access boundary. Discover profiles with system.users and assign with jobs.assign_owner({id,owner}). Original spec.owner, submitter, callback notify and execution snapshots are preserved separately. Results inherit the current Job Owner at publication and can later be reassigned through results.assign_owner.

## Output storage

Read `system.status.data_stores`, then set `output_storage` on `jobs.submit`, for example `"output_storage":"data/1"`. Omission uses data/0. Loom freezes the selected store identity for the Job, creates its `runs/<job>/<attempt>` workspace there and passes `/work` as `request.output_dir` to containers. Preparation, run, finalization, callbacks and logs follow that workspace. The producer keeps writing relative paths in result.json; it must not construct host paths or edit the DB.

Loom validates the files/hashes and indexes the selected storage ID, relative paths, metadata, metrics and lineage. Dependent Jobs can use `{ "job":"<producer-id>", "entity":"<output-id>" }`; registered locator inputs work across stores too. Inputs are mounted read-only, without mounting their entire store. `runtime.storage` is a separate opt-in for general shared RW access, not the output selector.

A missing/replaced store fails that Job instead of falling back to data/0. Result-indexing retries use the same output store and do not rerun compute. Recovery cannot change output_storage on an existing Job; submit a new Job to choose another output store. Image updates preserve configured paths and markers. Older installations expose data/0 by default; additional output stores require their persistent storage IDs and executor grants to be configured, not just an Explorer mount.

## Reconnect a restored artifact

After restoring the exact bytes of an existing artifact, use `results.reconnect`
with `id`, `revision`, the original `component` reference, the new store-relative
`ref`, and optional `storage` (default data/0). Discover the exact schema first.
Loom checks the registered store, path, immutable hash and size, then records the
location. This neither republishes different contents nor reruns the Job. Readers
cannot reconnect; ordinary users with submit scope can. Use `results.get` to
inspect the registered locations afterwards.
