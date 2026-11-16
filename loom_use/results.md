# Publish and reconnect results

A producer writes files and `result.json` under `request.output_dir`. The Profile declares output and metadata contracts. Inspect `system.contract` and the actual Profile before writing a manifest; generic success does not supply domain metadata.

Execution return, result publication and event ACK are separate facts. Use `jobs.get` for the Attempt and result state, `results.list/get` for native entities and locations, and `events.list(kind=all)` for notifications. A result-indexing error uses `results.reindex` after repairing publication; it must not rerun successful training.

Declare measurements, units, denominators, splits and provenance in the producer's output. Reference the accepted Profile revision and source hash. Large outputs can use the Rust SDK's sealed file inventory; see `authoring` and `items`. A structural/byte check does not establish model or dataset semantics.

Consume published artifacts through the Profile's accepted `{job,entity,member?}` or exact artifact input shape. Keep original producer and consumes lineage. Current Owner is responsibility metadata and can change without rewriting the original execution.

`results.list` includes imported historical entries with `record_type: historical` and `history_ref`. Pass that exact selector to `results.historical`; native entries use `results.get`. Recorded historical availability and current copy availability are distinct.

## Reconnect a restored artifact

After restoring the exact bytes of an existing artifact, use `results.reconnect`
with `id`, `revision`, the original `component` reference, the new store-relative
`ref`, and optional `storage` (default data/0). Discover the exact schema first.
Loom checks the registered store, path, immutable hash and size, then records the
location. This neither republishes different contents nor reruns the Job. Readers
cannot reconnect; ordinary users with submit scope can. Use `results.get` to
inspect the registered locations afterwards.


For a historical additional copy use `results.bind_historical_location` or the bounded `reconciliation` workflow. Reconnecting bytes, importing an external asset and creating a new scientific output are different operations.

## Output names

For new outputs, set a human-readable top-level label before publication.
Use `<subject> · <variant> · <purpose> [· <revision or selected step>]`.
Omit unknown or irrelevant parts; never invent counts, stages or a selected step.
Prefer English and keep the label under 120 characters (the SDK helper allows 200).

- Dataset: Grasp · RGB-D · Demonstrations · v2
- Model: X-VLA · RGB-D · Grasp policy · step 20000
- Experiment: X-VLA · Closed-loop evaluation · test96 · r3

Include the discriminators needed to distinguish sibling outputs: modality,
model family, protocol/cohort, checkpoint role or revision as appropriate.
Keep full parameters, counts, splits, timestamps, hashes and Runtime pins in
their typed metadata/provenance fields. IDs and file paths remain independent.
Do not use a Job UUID, an output local key such as checkpoint, or the Task name
alone as the display name.

Task membership is a separate exact `scenario_ref`/context link, shown as a property
in Outputs. It never replaces `label`. Reuse an accepted Task when appropriate;
do not create another Task just to name an output.

With the Rust SDK, register the file/entity or inventory, then call
`producer.set_entity_label(local_id, label)` before `finish`. For external imports,
use `definition.label` with the discovered registration schema. A curation/import
workflow must preserve existing names unless the user explicitly requests a
rename; this convention is not a migration gate for legacy results.
