# Concepts

## Research and execution

A **Scenario** describes the task and protocol: what is being attempted, under which conditions, and how it is evaluated. It has immutable accepted revisions. Jobs and research records can link to an exact revision. A Scenario is not a scheduler and does not infer relationships from names.

A **Job** is one requested outcome, such as generating a dataset, training a model or evaluating a policy. Its **Profile** selects reusable execution behavior. Cases, shards and batches normally stay inside that Job as **Steps**. A Step is producer-reported progress, not a separate resource allocation or proof of success.

An **Attempt** is a real execution of a Job. Retrying execution produces another Attempt; repairing result publication must not rerun successful compute. A multi-profile outcome can use one workflow Job with separately scheduled execution nodes. Nodes retain their own Attempts and outputs; the parent does not invent an execution.

`jobs.get.state` is the native state. `status` is the public presentation (`processing` maps to `finalizing`, `complete` to `completed`, `result_failed` to `finalizing_failed`). Scheduling explains eligibility and waiting. Recovery records the repair lease and round; `recovery.state=executing` can coexist with `status=running`. Event acknowledgment records delivery, not repair ownership or resource return.

## Reusable software

| Object | Defines | Example distinction |
|---|---|---|
| Component | Job lifecycle implementation and reusable Profiles | Dataset generation stages |
| Tool | Software invoked within a Job | Planner, policy or hybrid runner |
| Profile | Reusable parameter/input/output contract, stages and exact software/Runtime combination | A generation or evaluation recipe |
| Runtime | Versioned environment used to execute software | Image, platform and dependency inventory |
| Dependency | An external software/source/package element of an environment | Python, Isaac SDK or a native library |

Profiles can share Tools and Runtimes. Changing a seed or input normally reuses a Profile; changing source, interfaces, stage commands or environment requires a new accepted revision. A Runtime does not explain the task or replace a Profile. Dependencies are installed under the root configured for the installation, not a hard-coded path. A recorded package inventory does not prove that every package is imported or that GPU/Sim behavior is accepted.

The Dashboard's **Runtimes** workspace groups Profiles, Components, Tools, Runtimes and Dependencies. MCP retains the `components` group for software operations. Built-in origin means shipped by Loom; Owner identifies current responsibility. Neither is an access-control boundary for shared workspace records.

## Records, bytes and discovery

Results preserve typed datasets, models, experiments, metrics and provenance. Native artifacts use exact artifact/revision identities; historical records keep their original source/kind/id/revision/snapshot identity. A current copy binding adds a verified location without replacing the historical observation.

Common **Item** IDs connect these objects for discovery. **File** Items identify registered bytes; copy locations identify where those bytes can currently be read. **Log streams** identify exact Attempt/stage/source generations. Their large decimal IDs are strings. They do not replace native Job IDs, artifact references or input schemas.

Search is a projection and can lag. An empty partial search does not prove absence. Exact native reads and accepted artifact inputs remain separate from search. Reconciliation plans explicit adoptions, imports or matching copy bindings; it does not guess scientific meaning, rebuild an entire filesystem or fabricate an execution.

Research/Operations and main/support/test describe presentation. The current queue shows actual resource consumers regardless of these labels. Filters can reduce catalog noise without erasing execution history.

See [task guides](../loom_use/entry.md), [API reference](reference/README.md) and [implementation ownership](IMPLEMENTATION.md).
