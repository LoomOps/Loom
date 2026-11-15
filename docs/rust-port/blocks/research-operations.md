# Research / Operations, queue visibility and history

Status: redesigned cross-cutting behavior. Phases D1/D4/D5/D6. The reference
includes the queue/history correction `07c90e4`; this is source behavior, not a
claim that any installation has received it.

## Independent dimensions

| Fact | Meaning | Must not determine |
|---|---|---|
| scope: research / operations | Declared purpose | Eligibility, access, resource occupancy |
| role: main / support / test | Presentation/result role | Real workflow membership or scientific acceptance |
| execution membership | Durable Job-plan node edge | Inferred parentage from a text label |
| consolidated source | Explicit verified preserved execution reference | Rewritten original Attempts/times |
| category/mode | SDG/evaluation/etc. and full/pilot/smoke | Owner permission or whether it is research |
| Scenario association | Versioned task context | Automatic orchestration or queue hiding |

Do not infer purpose from names such as smoke, pilot or test. An explicitly
requested research diagnostic is research; platform installation/runtime acceptance
is operations even when it generates a technical artifact. Defaults are explicit
and visible; curation preserves reason/key/source and never rewrites sealed facts.

## Current queue

Read actual scheduler/Executor ownership and waits before any catalog curation.
Show all active, waiting, preparing, finalizing or paused occupants according to
existing queue contract, including operations/support/test. A parent summary may
fold real internal nodes but shows counts, actual resource occupancy and expandable
state. A standalone predecessor remains its own real Job unless it was submitted
as an explicit internal execution. Do not retrospectively hide it as a stage.

The fact that a row is minor to research browsing is insufficient to hide a GPU
occupant. Global Research/Operations/All catalog filters and terminal-history
filters do not filter current occupancy. Foreign-scope privacy remains respected;
show only the authorized/minimal shared occupancy surface already allowed.

## Terminal history and work units

Default All; provide independent scope, status, category and mode controls. Research
views may offer curated main outputs, but complete authorized source/operations
history remains drillable. Only authoritative workflow membership folds a node;
presentation.parent_job is a reference, not execution identity. Consolidated-source
rows are opt-in in summary lists without duplicating their parent summary.

One requested outcome is one Job. Batches/vectorization/parallel workers stay in
that plan; output/time limits do not justify a top-level Job per batch by default.
Independent requested outcomes can be separate Jobs. Already fragmented work keeps
its original identities and outputs, with explicit consolidation/continuation
rather than fabricated Steps or a fresh timestamp covering old execution.

## Software visibility and reuse

Research/Operations catalog selection may hide platform fixtures in default research
browsing while preserving Built-in discovery and complete inventories. Dependency
summaries show representative installed requirements; exact inventories remain on
demand. Usage counts are distinct logical Jobs with a recorded exact pin, separated
by purpose/mode and excluding duplicate retries/imports. A reused package is not
necessarily a meaningful main research result. Curation controls display only.

## Commands and projections

Use typed Presentation/HistoryQuery/QueueProjection, immutable request declarations
and additive reasoned curate receipts. The same Rust projection rules serve HTTP,
MCP and Dashboard. Submission acceptance stores declared purpose with its plan;
changing an annotation does not change accepted execution/ownership.

| Case | Trigger | Required result |
|---|---|---|
| VIS-01 | Operations or minor support Job holds a resource | Visible in current queue regardless of history/catalog filter |
| VIS-02 | Main/follow-up has presentation parent but no membership | Remains individually visible; actual nodes fold only by edge |
| VIS-03 | Thirteen prior executions consolidated | Honest source Steps/Attempts/times; no invented single Attempt |
| VIS-04 | Platform smoke plus requested research smoke | Different declared purpose; no name heuristic |
| VIS-05 | Profile reused over retries and many internal nodes | Count actual distinct logical Job use with exact pin evidence |
| VIS-06 | History filters changed | Independent filters affect only terminal history, default All |

Tests must include real service projections and Firefox; a pure filter unit test
cannot prove the active queue includes records removed earlier in a catalog path.
