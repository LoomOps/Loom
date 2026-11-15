# Scenario — detailed data and behavior design

Status: new proposed feature. Phases D1/D5/D6. It has no implemented Scenario
endpoint/table in the frozen reference. Existing `presentation.experiment` and
request_ref are useful evidence but are not a versioned Scenario identity.

## Responsibility

A Scenario describes the task/research setting and protocol: what is attempted,
initial conditions/demonstrations, relevant scene/assets, allowed variation and
success/evaluation criteria when recorded. It groups/references work without
executing it. Job is one requested outcome; Profile is a frozen software/stage
combination; Runtime is an execution environment; experiment remains a result
entity. A coordination Task and scheduler stage are not synonyms for Scenario.

A grasp/alignment task can have a Scenario with several protocol revisions and
research iterations. Generation, training and evaluation may be separate requested
Jobs sharing that exact context. A later task may reuse a dataset or model; this is
an additional link, not a copy or proof that the artifact was produced for it.

## Proposed schema and exact references

| Record | Key / fields | Invariants |
|---|---|---|
| scenarios | id, display label, current_revision, created/by, archived | Stable ID; optimistic expected revision; archive preserves links |
| scenario_revisions | (scenario_id, revision), definition JSON, hash, created/by, reason | Immutable definition/source; no in-place protocol overwrite |
| scenario_links | link_id, scenario revision, target kind/qualified identity, relation, provenance/evidence, created/by, superseded_by | Additive/explicit supersession; never rewrites target provenance |
| accepted Job context | exact Scenario revision/hash inside accepted plan | Resolves atomically at submit; later Scenario updates do not repin it |

A definition includes concise label/description and optional versioned protocol
metadata (initial conditions, action/object/scene/frame, success criteria,
evaluation definition and referenced demo/protocol assets). Unknown scientific
metadata remains typed-envelope JSON, not generic scheduler flags or inferred
truth. Core validation checks structure, size/locators/references; research-specific
semantic validators stay with their Profile/Component.

Targets use a discriminated reference: logical Job, exact native entity revision,
fully qualified historical source/kind/id/revision/snapshot/file/component identity,
exact Component/Tool/Profile/Runtime revision or protocol artifact. Preserve source
namespaces; a string ID alone cannot join historical and native records safely.
Relations distinguish produced-for, used-by, demonstrates, evaluates, implements
and contextual-reference semantics. Exact vocabulary/schema is frozen in D1.
A relation cannot claim a successful evaluation merely because a file exists.

## Commands and concurrency

- RegisterScenario(key, definition); UpdateScenario(key, id, expected_revision,
  definition, reason); ArchiveScenario with a revision fence.
- LinkScenario(key, exact_revision, target, relation, provenance) and explicit
  superseding/unlink annotation; never destructive history rewrite.
- Read/list/graph with bounded pagination, purpose and target-kind selectors.
- Submit Job/workflow optionally accepts scenario_ref and resolves an exact revision
  during the same acceptance transaction. Caller can deliberately omit it; old
  records remain unassigned rather than receiving a fabricated context.

All commands check authenticated scopes/delegated write policy and target grants.
Scenario visibility does not grant access to a referenced private file. The link
may expose a permitted unavailable/redacted target rather than leaking its details.
Stable keys replay the same result; changed inputs conflict. Two updates to the
same expected revision produce one winner and one actionable conflict.

## Existing data and reconciliation

Discover existing presentation experiment/request references, actual Job/result
lineage and exact source metadata as candidates. Provide a dry-run association
proposal with evidence and unresolved/ambiguous cases. Commit only explicitly
confirmed links through normal commands. Do not bulk classify by project/run
names or automatically merge protocols, Jobs, datasets or experiments.

Bounded link validation/reconciliation is part of this feature. A general agent
that autonomously interprets every record and repairs state remains deferred.
Original history, scientific metrics and selected checkpoints are untouched.
Historic context linked now is displayed as a later association, not an original
submission pin. No new generation/training Job or dataset count is manufactured.

## Query and UI

Expose Scenario summary/detail with concise definition/revision and linked Jobs,
datasets, models, experiments and exact software combinations. Show actual lineage
and separate confirmed context links. Unassigned and reusable assets remain
searchable. A filter narrows discovery/history; it cannot hide current queue
occupancy or change scheduler eligibility. Scenarios do not auto-create a pipeline.

## Acceptance

| Case | Trigger | Required result |
|---|---|---|
| SCN-01 | Protocol revision updated during/after Job acceptance | Existing Job pins original revision/hash; new Job explicitly chooses revision |
| SCN-02 | Dataset/model reused in two Scenarios | One artifact identity/count; two truthful typed links |
| SCN-03 | Historical/unassigned/ambiguous record | Discoverable, originals preserved, no automatic association |
| SCN-04 | Concurrent updates/link replay | Expected-revision conflict or original stable receipt; no duplicate links |
| SCN-05 | Cross-scope target or unavailable file | No privilege expansion; explicit unavailable/redacted evidence |
| SCN-06 | Archive or correct a mistaken link | Old evidence retained with explicit supersession; no scientific rewrite |

Review results/catalog/research-operations/API and the [migration](../MIGRATION.md)
plan together. New tables/actions/UI and migrations require dedicated Rust and
independent protocol/browser tests; generic catalog tests alone are insufficient.
