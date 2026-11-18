# Job boundaries and execution ownership

Status: design record. Outcome creation, plan revisions and Runtime receipt gates are implemented in the development tree. See the [current wire contract](../reference/jobs.md) and [submission guide](../../loom_use/workflows.md#accept-an-outcome-before-installation). Baseline: Loom 0.2.0-alpha.48, API v57, Loom Use alpha.14 and Pale 0.8.4. The action names and fields under “Proposed API” are design sketches.

## What counts as one Job?

A Job is the smallest independently requested execution outcome for which the caller can say what will finish the work. Give that outcome one stable ID and one completion event. Some outcomes complete when the sealed plan succeeds. Promised artifacts and Runtime receipts have typed acceptance requirements. A Job may publish several artifacts, consume several Runtimes and occupy several machines or lanes where the executor supports them. Execution placement remains an executor capability.

Apply the following test before designing a graph: if an intermediate operation succeeds and everything after it is abandoned, has the caller received a deliverable they independently requested? If so, it can be a Job with its own acceptance contract. If its only purpose is to make the promised outcome possible, keep it inside that Job. Reuse elsewhere does not retroactively change its original purpose. A request for a dataset, a trained policy and an evaluation can therefore produce three linked Jobs. If the request is solely for an evaluation and the other work only prepares its inputs, one Job can own those Nodes. Task/scenario and experiment families remain browsing and research context across Jobs.

| Object | Owns | Example |
|---|---|---|
| Job | Requested outcome, required deliverables or receipts, owner and terminal decision | Publish one verified dataset of 140 accepted cases |
| Execution Node | One independently schedulable unit under exactly one Job, with a pinned Profile, inputs, budget and retry boundary | Generate shards on separately allocated lanes, then assemble and verify the manifest |
| Stage | Runner lifecycle within an execution, including prepare, run and finalize | Release the GPU when the run returns |
| Attempt | Actual execution or retry recorded by the scheduler | Retry only the failed shard and keep its earlier evidence |
| Step | Producer-reported progress within an execution. Existing steps.json IDs identify named milestones or batches | 87 of 140 accepted cases |
| Related Job | A separately accepted outcome, optionally consuming an exact output from another Job | Evaluate a published policy under a new protocol |

A single-run Job has one execution. A workflow gives each independently scheduled execution a Node ID. The single execution uses the existing Job ID and needs no synthetic workflow membership. A graph has several scheduled Nodes. Add a Node when a separate allocation, Profile, restart boundary or publication dependency is needed. A Stage can choose another Runtime while retaining its execution identity. Same-Runtime Nodes can run independently. A Step report never dispatches work. A loop, a different Tool invocation, or a line written to a progress file is sufficient as progress within the same Node. Queue position, wall time, category, owner and the number of Pale Tasks cannot determine this boundary.

## Why the current data is easy to misread

- [Submission](../../rust/crates/core/src/jobs.rs) already has atomic workflow creation, a DAG, pinned child specs, output aliases and ordinary execution recovery. Single submissions lack a distinct outcome contract. A workflow can declare no public aliases.
- [Job reads](../../rust/crates/core/src/query.rs) expose physical membership from workflow_nodes. The family query also follows presentation.parent_job. [Activity](../../rust/crates/core/src/activity.rs) resolves the top of these links and unions execution intervals. A valid “shown together” relation can therefore change counting without changing either execution's completion.
- [Runner progress](../../loom_use/workflows.md) comes from a bounded steps.json file. It cannot prove that resources returned, a stage completed or a scientific output passed validation.
- [Software relations](../../rust/crates/core/src/graph.rs) can trace a Job through a pinned Profile to Components, Tools and stage Runtimes. A Tool pin establishes availability. [Result publication](../../rust/crates/core/src/results.rs) and [reporting](../../rust/crates/core/src/reporting.rs) already verify different evidence: producer, bytes, lineage and required reports.
- Pale 0.8.4 carries a coordinator's explicit continuation context across Tasks. Its legacy Main reference selects the earliest accepted binding within the outcome. The development adapter records a durable explicit binding for a Loom plan root and prefers it over earlier preparation. If the first executable work is a prerequisite, arrival order alone cannot establish what the user commissioned.

## Relations the server can enforce

| Relation | Authority | Observable effect |
|---|---|---|
| Job owns Node | Accepted execution plan | Only the owning Job can count, control or derive completion from this Node |
| Node requires Node | Acyclic plan within a Job | Scheduler readiness and failure blocking |
| Node consumes Output | Exact producer/entity and revision or verified existing locator | Readiness and reproducible input lineage across Job boundaries |
| Node uses software | Accepted Profile and stage pins, then execution receipts when available | Show planned Runtime, effective Runtime/image, lifecycle Component and pinned Tools separately. Record actual Tool invocation only with a receipt |
| Job promises Output or receipt | Frozen acceptance contract | Map each promised alias to an exact producer Node and verified evidence |
| Job relates to Job | Explicit follow-up, shared Task/scenario or historical display relation | Navigation. No transfer of ownership, retries, cancellation or acceptance |

A reusable Rust builder illustrates the split. If a compatible builder is installed, resolve its Runtime revision and grant for the compile Node. The compile Node selects a pinned Profile. Its run Stage carries the builder Runtime pin, and the build receipt identifies the resulting binary. The generation Node can use that binary under a different Runtime. Reusing an existing verified build needs no new compile execution. If installation is needed solely for dataset generation, give the same dataset Job a prerequisite for the verified installation receipt, then compile, generate and publish. The Runtime registry owns the installation and host authorization. Waiting for an administrator's receipt takes no compute lane. If a reusable builder is itself the requested delivery, create a Job accepted on the exact registered revision and technical validation receipt. Many later Jobs may use it without reparenting the installation. CPU-only planning produces a plan or document, with no placeholder execution.

## Outcome acceptance and controls

Freeze the deliverable identities, verification requirements and requested protocol when opening a Job. A publishable deliverable identifies its entity kind, producer alias and existing reporting/template contract where applicable. An operational deliverable references a typed receipt such as an installed Runtime revision or build digest. Auxiliary logs, pilot weights and comparison summaries remain attached to the Node unless explicitly promised and eligible for their catalog type. Two aliases can refer to the same producer revision without duplicating the artifact. A completed evaluation can report zero successes: the acceptance check verifies that the requested protocol was executed and reported. A scientific threshold is relevant only if explicitly part of that contract.

Derive the Job decision from the complete persisted plan. Completion requires a sealed plan, successful required Nodes, satisfied technical gates and verified bindings for every promised deliverable. Waiting on installation, recoverable execution failure, and pending publication expose the blocking Node and reason. A failed finalize can retry verification/publication without rerunning compute. Preserve current resource-return, callback and early verified-input rules. A hold prevents eligible new work from starting. A cancel targets only owned execution and reports per-Node outcomes until resources return. A completed Job stays completed. A new question after closure gets its own linked Job. Repeat delivery with the original request key returns the original result.

## Proposed API and storage

1. **Read first.** Add a discriminated object to jobs.get for outcome and execution detail. Return the owning job_id and workflow-local node_id for member executions, exact execution/attempt IDs, plus separate collections for prerequisites, related Jobs, progress and verified deliverables. Expand large collections with stable pagination. List responses return a compact logical Job summary. Keep existing v57 fields and exact source reads.
2. **One normal create path.** Propose jobs.create with an idempotency key, outcome goal, typed deliverables and a plan of named units. A plan unit is either a scheduled execution with an exact Profile pin or a receipt prerequisite with a typed verification target. An executable Node selects a Profile revision. A Job caller that needs different stage pins selects another compatible Profile or registers a revision. Each configured container Stage resolves its environment through that Profile. Job submission and progress Steps have no independent Runtime override. A direct image, legacy Runtime ID or built-in runner retains its actual recorded identity. Inputs, output store and resource limits retain their existing contracts. Ordinary callers submit a sealed single-Node or multi-Node plan in one transaction. A client never supplies an authoritative execution parent in a presentation field.
3. **Late prerequisites.** For an outcome whose Runtime or Profile does not yet exist, allow an open plan with promised aliases and stable execution slots, and executable prerequisites whose pins already resolve. An unresolved downstream slot has no dispatchable record. A receipt gate may unlock that slot once an authorized installation supplies the exact revision. Propose jobs.revise_plan with expected revision and key: append prerequisites or bind unstarted slots, validate the whole DAG and acceptance mapping, then seal. Preserve accepted pins, started work and the original acceptance criteria. A sealed plan with an unforeseen prerequisite can get an audited new revision while nonterminal, subject to the same guards. An amendment cannot remove a required check, add a dependency to dispatched work, replace a published binding or turn failed work into success. The scheduler and revision writer serialize readiness checks with admission. Once terminal, use a related Job. Expose an open plan awaiting input as such, including who or what can unblock it.
4. **Persist only new facts.** Add outcome and plan-revision records keyed by the existing Job identity, required deliverable definitions, receipt bindings, and membership for newly planned Nodes. Reuse workflow_nodes for physical membership and the native scheduler for Attempts, stages, recovery and resource accounting. Make (job_id, node_id) unique, and allow at most one owning outcome per physical execution. Write the outcome, accepted revision, new execution rows, edges and operation receipt atomically. Use unique (installation, submitting principal, request key) for new creates and an outcome-scoped key for revisions. Exact replay precedes revision checks. Reject changed replay, stale revision, cycles, invalid/foreign members, unresolved pins on dispatch, incompatible aliases and cross-installation receipt substitutions. Check existing action scopes and receipt authenticity. Approval for installation remains with the Runtime subsystem.
5. **Choose explicit identity at the caller.** On a new request Loom Use creates the outcome before any preparatory execution. Persist the returned Job ID in Pale's outcome context. A request key handles retries. Independent clients that deliberately coordinate must share a Job ID or the same supported idempotency scope. Different requests with similar titles remain distinct. Pale can observe an owned Node for updates without claiming it as an independently bound Main. One terminal notification per outcome is deduplicated by source, Job and terminal event identity. Node execution events remain available for recovery. Producer Step observations remain progress evidence.

Illustrative proposed create body (symbolic pin and entity values):

```json
{
  "key": "dataset-release-1",
  "outcome": {
    "goal": "Publish the verified 140-case dataset",
    "deliverables": [
      {"alias": "dataset", "kind": "dataset", "from": {"node": "publish", "entity": "dataset"}}
    ]
  },
  "plan": {
    "sealed": true,
    "units": [
      {"id": "generate", "execution": {"profile": "<generation pin>"}},
      {"id": "publish", "depends_on": ["generate"], "execution": {
        "profile": "<assembly pin>", "inputs": {"cases": {"node": "generate", "entity": "cases"}}
      }}
    ]
  }
}
```

In this example the assembly Profile must pin a verifier for exactly 140 accepted cases, or bind 140 as a typed parameter to its verifier at admission. The verified manifest provides the measured count. The free-text goal cannot enforce a count. The server can reject a plan without a compatible verifier. It cannot infer whether two independently declared goals should have been one request. A reused Runtime, similar title or coincident time must never be enough to rewrite ownership.

## Compatibility, surfaces and release sequence

| Slice | Implementation and acceptance |
|---|---|
| 1. Add typed reads | Domain/contracts and core query expose single, workflow, and legacy evidence through one projection. Keep v57 jobs.submit, jobs.submit_workflow, old IDs, original idempotency hashes and params working. Compare reads for all three forms before changing any default. |
| 2. New submissions | Native admission uses the existing transaction, plan ordering, Profile resolution, readiness and reporting validators. Exercise single execution, parallel shards, open plan with an external receipt, concurrent revisions, failed publication, stale/replayed requests and restart after commit. Verify no orphan records or double dispatch. |
| 3. Queue, history and Activity | Show one top-level row per outcome with running/blocked Node and actual lane use in the expanded queue. History records acceptance and related follow-ups separately. Activity counts one Job per active KST day, unions overlapping owned execution intervals, excludes pure waiting and labels unknown time. Independent Jobs sharing outputs count separately. Show recorded catalog and software evidence even if its current registry entry is archived. |
| 4. Clients and notifications | Publish action docs, jobs/workflows guides and version discovery. Loom Use selects an existing outcome ID, creates a new one, or returns a document for planning-only work. Installing a missing Runtime follows the authorized host workflow. On older servers without plan revisions, clients keep installation evidence with the original coordination outcome and submit runnable work only when its Profile is ready. Pale uses explicit Main IDs where supported and its existing conservative fallback on v57. Test two Tasks, one outcome, one terminal Master report and an independent linked evaluation. |
| 5. Reconcile existing records | Dry-run on a bounded sample: simple Job, real workflow, fragmented generation, presentation-only follow-up, shared Runtime and historical missing evidence. Derive exact membership from workflow_nodes. Expose consolidation as reuse of the identified source Attempt. Label presentation-only families as legacy groups and retain their old browsing path. Establish ownership for ambiguous historical rows only with reviewed evidence and reversible overlays. Retain original timestamps, entities, IDs, unknowns and exact lookup. |

Roll out the read projection in shadow first. Compare today's Home, Activity, queue and result-lookup answers against exact receipts. Explain any count delta per Job before switching defaults. Enable new writes after API/MCP parity and cross-version client tests pass. Rollback can disable new writes and return the previous read default while retaining new plan records. Once new plan writes exist, keep a reader/executor compatible with those records until they drain or are explicitly restored from a consistent snapshot.

The first implementation slice is independently useful: the same answer to “what did I ask for, which execution is running, what did it use, and where is its result?” on Job detail, API and MCP. Subsequent slices can change submission and counting without rewriting historical execution.
