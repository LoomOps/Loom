# Jobs and Units

Read [worked requests](job-cases.md) when an outcome spans several executions. Use the connected server's `system.status`, `system.guide` and `system.describe` for discovery.

- A **Job** delivers one independently accepted Regular Dataset, Policy or Experiment. Use `jobs.submit_research` with a deliverable alias and the Unit that publishes it. Link separate outcomes through exact upstream outputs.
- A **Unit** runs a pinned Profile. Put prerequisites, partial batches and verification in the Job that needs them. Use `units.submit` for independently requested setup, Pilot, maintenance or development. Standalone Units can publish Reference evidence.
- An **Attempt** is one try of a scheduled Unit. Rerunning creates a new Attempt. Repairing publication keeps the successful Attempt. Producer-reported progress appears under the running Unit and shares its resource grant.
- A **Stage** is the engine's prepare, run or finalize lifecycle within one Unit. Unit phase describes its role in the research plan.

## Resolve submission inputs

1. Start with the latest user request and relevant prior decisions. Use exact prior Job output and catalog revisions when asked to repeat accepted work.
2. Inspect the selected Profile and verify parameters, defaults, required inputs, expected outputs and software pins. Review the closest reference for call shape.
3. Plan resources, dependencies and batching from the available Profiles and requested outcome. Ask the user when a required choice changes the cohort, environment, budget, protocol or acceptance. Defer only the affected submission.
4. Reuse the original key and arguments after an uncertain reply. Keep returned Job, Unit and eventual Attempt IDs.

## Queue and resource control

`jobs.queue` exposes waiting reasons and available controls. Job Queue aggregates scheduled member Units. Unit Queue shows standalone and member Units. Terminal history contains one record per Job or standalone Unit. Exact detail remains readable after departure from the live queue.

Use `units.get` for execution, `units.hold` or `units.resume` for pending work, and `units.cancel` for cancellation. A hold takes effect at a stage boundary. Use `units.reorder` for eligible waiting Units. Explicit same-device exclusive reservations retain their accepted order. `jobs.cancel` on a research Job requests cancellation of its scheduled Units. Check state and process return afterward.

Use an existing CPU Profile for CPU preparation. Use `gpu_access.mode: "shared"` only when the work can coexist and the Executor supports a configured memory reservation. Supply `memory_mib` and the applicable device IDs. Use `gpu_access.mode: "exclusive"` when the work requires sole access to its selected devices. Exclusivity lasts through confirmed return of the process or container. Host device guarantees cover Executor-managed workloads. Review live host policy and resource capacity before promising availability.

Pin `output_storage` per Unit when a store other than data/0 is needed. Use the configured selector and retain the registered storage identity. Producers write relative paths under `request.output_dir`. Any successor can use an exact artifact revision or the documented producer input shape.

For an older installation without `job-unit/v1`, use its advertised Job operations and retain its original IDs and accepted contracts.
