# One outcome, multiple execution units

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

## Accept an outcome before installation

Discover `jobs.create` and `jobs.revise_plan` on the connected server. Create one Main Job per independent requested outcome. Put prerequisite work in that Job's units. Provide a stable key, owner, notify target and goal. Declare typed deliverables for promised artifacts or Runtime verification. Each artifact alias names its producing slot, entity and kind. A `runtime_receipt` alias names a host Runtime ID and revision. If execution success is the acceptance condition, omit `outcome.deliverables` or supply an empty array. A sealed plan must have executable work or a verified Runtime receipt. For an immediately runnable plan, submit all units with `sealed:true`.

If installation is pending, submit `sealed:false`. Declare the waiting slot's ID, label, dependencies and required Stage Runtime id/revision; omit its `execution`. Other bound units can run. Keep the returned root ID. The waiting slot has no Attempt, resource reservation or active time.

For a bounded history review use `jobs.preview_history` when advertised. Match
exact Jobs and Attempts before proposing consolidation. A presentation parent
is navigation evidence; an accepted workflow Node determines execution
ownership. Leave ambiguous or preserved records in review.

For a scientific outcome, call `jobs.preview_outcome` with the draft outcome before creation. Use `outcome.science.deliverables` to pin a template and report contract per alias. Keep the accepted Job ID and `science_digest`. Revisions bind execution to the existing contract. An exploratory outcome can omit the hypothesis. See [Reporting](reporting.md) for publication evidence.

After authorized host installation and Profile registration, read `jobs.get.spec.plan` and `plan_revision`. Submit the full next plan to `jobs.revise_plan` using that ID, expected_revision, a fresh operation key and a reason. Supply the slot's `execution.kind` (exact Profile revision) and params. The server compares each required Runtime with the granted host observation, passed validation and frozen Stage pin. Sealing a receipt-only outcome verifies its installation receipt without dispatching a process. A sealed plan cannot add or change an execution slot. Admit follow-up work as a separate Job if its completion requires a new plan.

On an uncertain response replay the identical request and key. On revision conflict read the current plan before making a new change. Failure to admit a binding leaves the prior revision usable. Keep admitted units and their dependencies. Completion requires all owned executions and promised outputs. Find published files through the output alias and installation receipts in `jobs.get.result.receipts`.

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

For a fully specified graph without an outcome acceptance contract, use
`jobs.submit_workflow` after reading its live schema. Submit `nodes` with unique
IDs, labels, pinned Profile specs and `depends_on` node IDs. Internal inputs use
`{"node":"producer","entity":"output"}` and must name a declared predecessor.
`outputs` maps public aliases to exact node/entity references. A later consumer
may use `{"job":"<main-id>","entity":"<alias>"}`; submission resolves it to the
original verified producer, preserving lineage and dependency checks.

Select a Profile for each node. Its configured Stage runners determine their environments. A change of Runtime between prepare and run is part of that Profile. A producer Step reports progress within the execution and has no Runtime selector.

The complete graph is accepted atomically or rejected without orphan executions.
Replay its exact key/request after uncertainty. Each node uses the ordinary
scheduler, resource-return checks and recovery budget. The main Job has execution
nodes, no invented Attempt. Repair/log/reindex target the actual node; cancel or
hold/resume on the main Job delegates to its nodes. Per-node responses describe
partial control outcomes. Reorder on the main Job moves its remaining waiting nodes as one group; an actual node can also be selected. A failed node is exposed
in main Job errors; independent eligible nodes may continue. A main completion
requires every node completed; supporting receipts are not new research outcomes.

`presentation` records scope (research/operations), role (main/support/test),
optional label, experiment, request_ref and output_roles. An explicitly requested
research diagnostic can be main; a platform acceptance fixture is operations/test.
Use server-side roles instead of guessing from names, Owner or Built-in origin.
User-requested Runtime preparation, development and diagnostics are user work,
even when their files are not eligible scientific Outputs. Keep their requested
outcome `main`, category `development`; use `support` for prerequisite stages.
Reserve `test`/System for platform installation checks. Scientific output roles
do not determine whether the parent Job belongs in execution history or Activity.
`jobs.list` defaults to `view=all`; `results.list` defaults to Research.
Set `view=research` or `view=operations` explicitly for a purpose-specific list;
`view=all` retains full audit access. Direct reads remain.
The shared queue and Dashboard current queue always retain all real resource
consumers, independently of catalog or history filters. Dashboard terminal
history defaults to All and has separate Research/Operations, status, Job Type
and Execution Mode filters. Workflow executions remain grouped under their
logical Job; a presentation parent alone does not hide a standalone follow-up.

`jobs.curate` previews additive presentation/binding changes by default. Apply a
reviewed batch with dry_run=false and a stable key/reason. Originals, timestamps,
outputs and execution pins are preserved; applying another batch can restore the
previous overlay. Historical grouping does not reschedule or change dependency
readiness. Prefer an execution graph for new multi-stage work. Platform tests run
in isolated dev instances; production-only acceptance stays operational.

Profiles can record intended presentation.scope/role/output_roles. Pass Job
presentation explicitly on native submissions; the accepted request freezes it.
Without it, native submission defaults to research/main (cpu.smoke uses
operations/test). Component tags classify software, not the requested Job's
purpose. Later classification or display edits never rewrite submission identity.

Declare output_roles for each experiment-shaped output when a Profile produces
multiple experiments (for example an evaluation and a budget receipt). Without
these declarations, ambiguous experiment outputs remain supporting records on
the Job, rather than being counted as independent scientific experiments. Exact
records and input references remain available; a published workflow output alias
explicitly identifies the requested primary result.


For task/protocol context use the `scenarios` guide; source authoring uses `authoring`.

## Preparation and follow-up history

When a later preparation or repair serves the same requested outcome, preserve
the existing Main Job ID and record `presentation.parent_job` on the follow-up.
A new coordination task does not imply a new Loom outcome. Retain its own
`request_ref`, exact Profile/Runtime pins and upstream inputs. `jobs.get.family`
exposes this hierarchy, including failed Jobs and real workflow execution nodes;
`relation` distinguishes presentation from scheduler membership. Up to 200
members are returned with `total`/`complete`; exact Job reads remain available.
Grouping never changes completion, cancels descendants or proves Runtime reuse.
On physical Job reads, `execution_membership` identifies actual workflow nodes;
null means an independent execution even if it has a presentation parent. Do not
rebind workflow nodes or infer execution ownership from a browsing group.
Do not submit a dummy Job for CPU-only planning or code review; associate that
evidence with the existing task/outcome and reusable software instead.

Home group summaries describe related execution activity, not a reopened root
execution. Use each member's exact state and receipts for completion claims;
`family.summary` is bounded and reports whether its coverage is complete.
