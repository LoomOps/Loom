# Job and Unit boundaries

A Job accepts one independent scientific deliverable. One Job can contain a dependency graph of scheduled Units with different Profiles and Runtime pins. A Unit can also be standalone for setup, software development, smoke or Pilot work. Both kinds share the same execution and recovery engine.

| Boundary | Example |
|---|---|
| Separate research Jobs | Generate a reusable Dataset, train a Policy, then run a closed-loop Evaluation |
| Member Units within one Job | Prepare a scene, generate several batches, validate and publish a single Dataset |
| Standalone Unit | Build and validate a reusable Observer with optional Reference evidence |
| Shared preparation | Verify an existing archive once for several Jobs. Register the external Dataset with its import provenance and pin that revision in each Job. |
| Observed Unit | Generation batch or other in-process progress under the running Unit |
| Attempt | One try with its own execution and return receipts |

Use Regular for a Job's accepted Dataset, Policy or Experiment or a confirmed scientific import. Use Reference for catalog-worthy pilot or diagnostic evidence. Software revisions have their own registry. Comparisons over published results start with exact reads. Grouping comparable evaluations preserves each score, while a checkpoint supplement appends a revision to the same Experiment.

The public model uses `unit.job_id` for membership and exact IDs for dependencies. Existing workflow and presentation relationships remain readable under the historical contracts.

See [execution guide](../../loom_use/jobs.md), [plan guide](../../loom_use/workflows.md) and [worked requests](../../loom_use/job-cases.md).
