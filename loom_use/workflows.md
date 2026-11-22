# Research plans and Unit lifecycle

## Planning

Bind one Job's deliverable to `outcome.deliverables[0].from:{unit,entity}`. The Unit name is local to the plan. Its pinned Profile declares that entity and kind. Use `jobs.preview_outcome` to inspect optional scientific commitments using the preview action schema. Attach a hypothesis when the request defines one.

To add checkpoint cases to an existing Regular Experiment, follow [checkpoint supplements](reporting.md#checkpoint-supplements).

`plan.units` defines each Unit's local `id`, label, optional phase and prerequisite Unit names. Bound slots specify `execution.kind` and `execution.params`. Local inputs use `{unit,entity}` and name a declared prerequisite. A downstream Job consumes an accepted upstream result via `{job,entity}`, where `entity` is the producer Job's deliverable alias. An exact artifact reference can instead pin a published revision directly.

Use a sealed plan when all bindings are ready. When a Runtime is pending, reserve the same outcome with `sealed:false` and an unbound slot. Use `jobs.revise_research` with the exact Job ID, expected revision and preserved accepted slots to bind it. An unbound slot has no execution or Attempt.

## Progress and continuation

Keep episodes, optimizer updates and other in-process progress in the producer's existing progress report. The SDK writes the bounded `loom-job-steps/v1` compatibility report, which the Dashboard displays as observed Units on typed execution. In a typed run request, `unit_id` identifies the scheduled execution and `research_job_id` is its optional Job. Existing physical producer and Attempt fields retain their original meaning.

When intake pins a Task, the worker request carries `scenario_ref` with its ID,
revision and SHA-256. Read that pin to validate Task-specific inputs. Legacy
requests without an accepted Task pin can omit it. A successful process exit or
registered diagnostic artifact alone is insufficient to establish a completed
Unit. Check its terminal state and accepted outputs before advancing dependents.

For a timed phase with a stable total, report a cumulative counter via the SDK's
`report_work_progress` or the `loom-work-progress/v1` file. Declare
`progress_reporting: true` in a Profile with a reporter. Use one reporter per
running Unit, emitting an initial sample and then about every five minutes.
If an estimate is unavailable at entry, proceed with the Job. The producer may
report `unknown_total` or `unstable_rate` with `report_work_unavailable` and
start reporting measured counters later in the same Attempt. The Dashboard
estimates the reported phase; queueing, later Units and publication are separate.

For long generation, pin episode IDs, seeds, configuration and input revisions. Checkpoint completed episodes atomically with hashes and a durable manifest. On restart, verify the checkpoint against the accepted inputs and skip exactly the completed items. Use [recovery](recovery.md) for a failed Attempt or publication repair.

## Historical execution

Exact historical reads preserve workflow memberships, source Attempts and presentation links. Keep the recorded start and return times. Use `jobs.preview_history` to inspect evidence before requesting any legacy curation. Follow the connected server's advertised compatibility actions when operating an existing legacy record.
