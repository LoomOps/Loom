# Research plans and Unit lifecycle

## Planning

Bind one Job's deliverable to `outcome.deliverables[0].from:{unit,entity}`. The Unit name is local to the plan. Its pinned Profile declares that entity and kind. Use `jobs.preview_outcome` to inspect optional scientific commitments using the preview action schema. Attach a hypothesis when the request defines one.

To evaluate additional checkpoints for an existing Regular Experiment, submit a new Job with `outcome.deliverables[0].supplements:{id,revision}` targeting its current revision. Bind the same exact Policy, Dataset and evaluation inputs. Produce only the new checkpoint cases under the same protocol and seed/reset cohort. See [checkpoint supplements](reporting.md#checkpoint-supplements).

`plan.units` defines each Unit's local `id`, label, optional phase and prerequisite Unit names. Bound slots specify `execution.kind` and `execution.params`. Local inputs use `{unit,entity}` and name a declared prerequisite. A downstream Job consumes an accepted upstream result via `{job,entity}`, where `entity` is the producer Job's deliverable alias. An exact artifact reference can instead pin a published revision directly.

Use a sealed plan when all bindings are ready. When a Runtime is pending, reserve the same outcome with `sealed:false` and an unbound slot. Use `jobs.revise_research` with the exact Job ID, expected revision and preserved accepted slots to bind it. An unbound slot has no execution or Attempt.

## Progress and continuation

Keep episodes, optimizer updates and other in-process progress in the producer's existing progress report. The SDK writes the bounded `loom-job-steps/v1` compatibility report, which the Dashboard displays as observed Units on typed execution. In a typed run request, `unit_id` identifies the scheduled execution and `research_job_id` is its optional Job. Existing physical producer and Attempt fields retain their original meaning.

For long generation, pin episode IDs, seed/configuration and input revisions. Checkpoint completed episodes atomically with hashes and a durable manifest. Only claim restart from a checkpoint if the producer verifies that state and skips exactly the completed items. Resume pending dispatch with the corresponding hold control. Use the recovery guide for a failed Attempt or publication repair.

## Historical execution

Exact historical reads preserve workflow memberships, source Attempts and presentation links. Keep the recorded start and return times. Use `jobs.preview_history` to inspect evidence before requesting any legacy curation. Follow the connected server's advertised compatibility actions when operating an existing legacy record.
