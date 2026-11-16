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

Display-name curation changes current result, Item and Dashboard names without
rewriting accepted source manifests, historical bodies or file pins. An exact
source revision retains its identity when its display name changes; historical
source reads still show the recorded name. Keep Job titles and Task membership
separate from the output name.

## Research metadata and primary outcomes

Record dataset quantity/unit/split, observation modalities, source demonstrations,
physics/control configuration, annotation origin and instruction conditioning when
applicable. Distinguish measured observations, privileged simulator labels and
estimated observer labels. An absent language input is different from unknown.

For evaluations, record offline versus closed-loop scope, Sim versus Real,
protocol and success predicate/threshold/window, evaluator Tool/Component revision,
checkpoint, cohort/split and successes with their exact denominator. Keep invalid,
infrastructure-failed and pending cases separate. Mark a primary outcome only when
its population and definition are the intended research result. Multiple threshold,
phase, observer diagnostic or conditional success rates are separate metrics; do
not select their maximum, average incompatible denominators or call all primary.
Use the Profile metadata schema; unknown fields remain unrecorded. Research
acceptance is distinct from Job exit, artifact publication and runtime smoke.

For a scientific summary, resolve the exact Item then call
`system.dashboard_entity(item_id, revision_id)`. Read `research_summary.facts` and
their evidence instead of guessing from a label. It can recover existing verified
JSON manifests/readiness/protocol reports that were not flattened into metadata.

New Profiles should declare a `metadata.research` object in their output metadata
schema. Use explicit `environment`, `robot`, `physics` (physics_hz, control_hz,
rgb_hz, state_hz, collision, payload_mass_kg, robot_gravity_disabled), `annotation`
(method/source), `instruction` (mode/source), `collection` (count/unit/split,
source_demos/method), `model` (role/architecture/outputs/initialization/base_model,
training_method/selected_step/optimizer_updates/selection) and `evaluation`
(mode/evaluator/cohort/split/success_criteria/planned/valid/infra/pending) fields as
applicable. Do not fill irrelevant or unmeasured fields. Units are explicit.
Keep a compact JSON context/report as a registered output and reference its exact
artifact hashes; a path string alone is not a verified source. Record observed
physics/sensor settings rather than treating a requested recipe as readback.
