# Domain and persistence design

Names below describe typed internal concepts; existing wire IDs/status aliases
remain readable through adapters. String newtypes accept existing identifier
formats rather than imposing UUID-only validation on historical IDs.

## Aggregates and independent facts

| Aggregate / value | Identity and invariants |
|---|---|
| `Scenario` / `ScenarioRevision` | Stable research question/task description; immutable protocol revision, optional initial demos and success criteria; no execution state |
| `Job` | One requested outcome, stable submitter/key/request, immutable accepted plan revision and resolved Scenario revision when provided |
| `ExecutionPlan` / `NodeId` | Explicit DAG of resource/profile stages; real membership, no inference from presentation.parent_job |
| `StepId` / `StepObservation` | Producer progress within an execution; per-source sequence and recorded timing, not an extra scheduler Job |
| `Attempt` | One execution try; separate stage receipts, failure history and current-attempt fence |
| `ExecutorTicket` | Accepted launch identity, exact request/launch fingerprint, resource grant and observed return |
| `RecoveryIncident` / `Lease` | Actor, deadline, shared budget, expected Job/Attempt and repair scope |
| `Publication` / `EntityRevision` | Immutable artifact identity/bytes, validation and indexing evidence; execution success is a separate fact |
| `SoftwareRevision` / `ProfilePin` / `RuntimePin` | Exact source/build/composition/environment identity; display attribution is separate |
| `AvailabilityBinding` | Current verified locator for a historical identity; never rewrites recorded original availability |
| `Presentation` | Research/Operations and main/support/test display facts; never eligibility, permission or implicit membership |

Use enums for lifecycle, effect/receipt status, stage, failure reason and resource
ownership. Opaque params, scientific metadata, unknown accepted historical fields
and byte-exact sealed JSON may remain JSON. Do not turn the aggregate back into
`serde_json::Value` with string-key transition helpers.

## Job transitions and command outcomes

A Job has separate execution, publication and recovery facts. The wire adapter
projects current `state`/`status` aliases without conflating those facts. Accepted
execution proceeds queued -> preparing/ready -> running -> returned/finalizing ->
terminal publication state. Preparatory readiness does not count as GPU occupancy.
A finalization failure retries finalization against frozen execution inputs; it
does not allocate a new training Attempt or rerun admission/prepare.

Commands include SubmitOutcome, AcceptExecution, ObserveStep, RequestCancellation,
ObserveReturn, PublishResult, RetryFinalization, ClaimRecovery, ResumeExecution,
ReorderWaiting, SetHold and SetOperatorDispatchPause. Each defines allowed source
states, expected plan/attempt/epoch, actor rights, write set, effects and idempotency.
A cancellation request does not assert return. Late old-Attempt completion/return
cannot overwrite a newer Attempt. Cancellation wins the publication race while
retaining actual returned/output evidence. Terminal observations are idempotent.

Recovery resume clears the current-result projection while preserving prior
Attempt results. A previous failure beside an executing recovery is history,
not evidence the new Attempt is failed. Leases/automatic retries share the
existing server budget. Defer, operator pause, hold and resource wait remain distinct.

## One Job, internal executions and honest counts

Simple runs use one execution; multi-profile/resource pipelines use a pinned DAG
under the same logical Job. Nodes can hold real resources concurrently. Do not
count 140 generated items as 140 Jobs unless they are separately requested outcomes.
Attempts count retries of executions; Steps count actual producer/stage evidence.
A collector or continuation is not an invented Attempt for earlier work.

Consolidated historical executions retain their original Job/Attempt/source IDs and
output hashes. The logical summary includes evidence-backed earliest/latest times
and a union of active stage intervals, excluding queue/ready wait and overlapping
parallel time. Record source timing confidence; never fill gaps with fabricated
labels such as integration verification. Source details remain drillable.

## Storage layout and consistency

Preserve Ops DB/tables and Runtime DB ownership initially. Add Scenario tables,
bindings and a versioned durable effect/receipt journal additively; do not replace
all historical rows with a new schema. Within the Ops DB, Job acceptance, Scenario
pin, real DAG membership and effect intent commit atomically. Shared Runtime DB
operations use durable two-step intent/receipt and instance grants; no assertion
of cross-file atomicity or unaudited second writer.

Each persisted transition compares current Job, Attempt, revision and installation
epoch before applying. A monotonic internal version may be added only through a
migration with existing-row defaults and a fenced old writer. Read models derive
from the same committed facts, optionally cached with a revision watermark.
Rebuilding a cache cannot launch work, reset budgets or alter immutable evidence.

## Core transaction sketch

```text
authorized command + stable request key
  -> bounded admission -> begin transaction
  -> reload grants / epoch / aggregate / existing receipt
  -> domain decision -> rows + durable effect intent + response receipt
  -> commit -> external effect under stable identity
  -> verify observed evidence / expected Attempt / epoch
  -> commit receipt and next decision
```

A committed key replays the original result, including after recovery or profile
changes; a changed request conflicts. Exact legacy hash/request namespaces are
preserved. Retrying a key may return uncertainty; it does not automatically repeat
an unkeyed upstream action. Rust ownership does not eliminate external uncertainty.
