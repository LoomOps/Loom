# Jobs and Units

Use [worked requests](job-cases.md) when choosing the boundary of a new run.

- A **Job** delivers one independently accepted Regular Dataset, Policy or Experiment. Use `jobs.submit_research` with the required `scenario_ref:{id,revision}`, deliverable alias and publishing Unit. Link separate outcomes through exact upstream outputs.
- A **Unit** runs a pinned Profile. Attach prerequisites, partial batches and verification to the Job that owns them. Use `units.submit` for independent or shared preparation, Pilot, maintenance or development. Downstream Jobs pin its outputs as inputs. Standalone Units can publish Reference evidence.
- An **Attempt** is one try of a scheduled Unit. Rerunning creates a new Attempt. Repairing publication keeps the successful Attempt. Producer-reported progress appears under the running Unit and shares its resource grant.
- A **Stage** is the engine's prepare, run or finalize lifecycle within one Unit. Unit phase describes its role in the research plan.
- A **Pilot** is a reduced preflight for an identified full run. Set `execution_mode:"pilot"` with `pilot.preflight_for` and `pilot.reduced_scope` when that is the accepted purpose.

## Resolve submission inputs

Look up the Task with `results.scenarios` and reuse its exact revision. If the research task is established and missing from the registry, register it once with `results.register_scenario`. Ask which task the user means when the request, prior Jobs and exact inputs leave it unresolved. Pin the Task before submitting any Profile that declares Dataset, Policy or Experiment outputs, including a standalone Pilot. General infrastructure work can omit the Task and record its evidence as Artifacts or Runtime receipts.

1. Start with the latest user request and relevant prior decisions. Use exact prior Job output and catalog revisions when asked to repeat accepted work.
2. Inspect the selected Profile and verify parameters, defaults, required inputs, expected outputs and software pins. Review the closest reference for call shape.
3. Plan resources, dependencies and batching from the available Profiles and requested outcome. Ask the user when a required choice changes the cohort, environment, budget, protocol or acceptance. Defer only the affected submission.
4. Reuse the original key and arguments after an uncertain reply. Keep returned Job, Unit and eventual Attempt IDs.

## Queue and resource control

`jobs.queue` exposes waiting reasons and available controls. Job Queue aggregates scheduled member Units. Unit Queue shows standalone and member Units. Terminal history contains one record per Job or standalone Unit. Exact detail remains readable after departure from the live queue.

Use `units.get` for execution and `units.reorder` for eligible waiting Units. Holds take effect at a stage boundary. Apply hold, resume and cancel in the `jobs` group for a Job ID or in `units` for one Unit ID. After cancellation, check the affected Units and actual process return. Explicit same-device exclusive reservations retain their accepted order.

Use an existing CPU Profile for CPU preparation. Use `gpu_access.mode: "shared"` only when the work can coexist and the Executor supports a configured memory reservation. Supply `memory_mib` and the applicable device IDs. Use `gpu_access.mode: "exclusive"` when the work requires sole access to its selected devices. Exclusivity lasts through confirmed return of the process or container. Host device guarantees cover Executor-managed workloads. Review live host policy and resource capacity before promising availability.

Pin `output_storage` per Unit when a store other than data/0 is needed. Use the configured selector and retain the registered storage identity. Producers write relative paths under `request.output_dir`. Any successor can use an exact artifact revision or the documented producer input shape.

For an older installation without `job-unit/v1`, use its advertised Job operations and retain its original IDs and accepted contracts.
