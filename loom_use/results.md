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
Use `<robot model> · <subject> · <variant> · <purpose> [· <revision or selected step>]`.
Omit unknown or irrelevant parts; never invent counts, stages or a selected step.
Prefer English and keep the label under 120 characters (the SDK helper allows 200).

- Dataset: Franka Panda · Grasp · RGB-D demonstrations · v2
- Model: Franka Panda · X-VLA · RGB-D grasp policy · step 20000
- Experiment: Franka Panda · X-VLA · Closed-loop evaluation · test96

Robot is also a typed property, independent of the label and Task context. Use
the recorded model; retain unknown or inapplicable status explicitly rather than
inventing a name. Multiple-robot outputs declare their models without selecting
one arbitrarily. See `metadata` for registered formats and canonical field types.

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

Record dataset quantity/unit/split, stored observation fields, source demonstrations,
physics/control configuration and annotation origin. Dataset language means an
actual stored task/text annotation, with its field and granularity; a generator's
prompt or an observer-source identifier does not establish stored language or
depth. Keep stored observations, observer inputs and policy inputs separate.
For Models, record actual policy conditioning and input consumers (`used_by`),
including proprioception dimensions. An absent language input differs from unknown.
Do not author Overview prose for Outputs; keep useful information in typed fields.

Find the most comparable exact prior output before choosing a format. Reuse its
registered metadata template when compatible; copy its structure rather than
values. New or changed fields require template coverage. New Profiles bind exact
template revisions; legacy metadata_schemas remain an additional compatibility
check, not a substitute for the selected template contract. Read `metadata`.

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

The selected template defines `metadata.research`; a Profile may add
`metadata_schemas` for further producer constraints. Read its required fields and
recording statuses rather than duplicating a schema. Use explicit `environment`,
`robot`, `physics` (physics_hz, control_hz,
rgb_hz, state_hz, collision, payload_mass_kg, robot_gravity_disabled), `annotation`
(method/source), `language` (stored Dataset field/granularity), `instruction`
(Model/evaluation conditioning mode/source), `quality` (recorded BC eligibility
and positive coverage), `collection` (count/unit/split,
source_demos/method), `model` (role/architecture/outputs/initialization/base_model,
training_method/selected_step/optimizer_updates/selection) and `evaluation`
(mode/evaluator/cohort/split/success_criteria/planned/valid/infra/pending) fields as
applicable. Do not fill irrelevant or unmeasured fields. Units are explicit.
Keep a compact JSON context/report as a registered output and reference its exact
artifact hashes; a path string alone is not a verified source. Record observed
physics/sensor settings rather than treating a requested recipe as readback.

To persist recovered facts on an existing record, preview
`results.refresh_metadata` with the exact entity/historical `target`, `reason` and
`dry_run:true`. Apply with the returned `expected_source_sha256`, `dry_run:false`
and a stable `key`. This adds a source-bound annotation; it does not revise a Model,
Dataset, accepted manifest, historical body or scientific acceptance. Keep missing
facts missing. Repeat only for the requested records, not every host file.

## Dataset derivation

Search existing Datasets and inspect their exact Item/source revisions before
publishing a variant. Reuse the existing Dataset when only its location changes.
A subset, relabel, sensor rerender or merge is a separate Dataset; a shared Task
or a calibration/tool input alone is not a data-parent relation.

In a native Dataset output, declare `lineage` with one entry per accepted
Dataset input. This is an illustrative fragment, not a complete runnable manifest:

```json
{"lineage":[{"input":"parent_dataset","transformation":{"kind":"subset","details":"Selected held-out conditions","member_count":20}}]}
```

Finalization resolves `input` from the frozen accepted Job inputs to one exact
Dataset revision. It keeps ordinary `consumes` edges separately. Supported
transform kinds are discovered in `system.describe`/the results reference;
condition selection is not sensor recording, and partial reuse does not mean the
whole target is a copy. Pin the producing Tool/config in the transformation when
applicable. With Rust, call `Producer::derive_dataset` after registering the output.

For existing native or historical Datasets, preview `results.link_dataset` with
exact `target` and `parent`, `transformation`, `reason` and registered component
`evidence` (target/component/SHA-256, optional JSON pointer or archive member).
Apply using its `expected_digest`, a stable `key` and `dry_run:false`.
The server validates exact identities, registered component hashes and acyclicity.
Transform semantics and any member-level comparisons remain the submitting
producer/operator's evidenced claim, not an automatic scientific verification.
An archive-member digest is retained evidence, not a claim that the API has read
every archive member. Existing source bodies/IDs/Jobs remain unchanged.

Browse `dataset_lineage.parents` in result/details and `results.item_graph` for
exact edges. Dashboard groups loaded variants while preserving their names and
counts; it never sums overlapping episodes or treats every consumed input as a
data parent. Multi-parent variants retain every parent cross-link; optionally
declare one `primary:true` parent for browsing. Shared context is not derivation.
Imported copies share a browse row only when the original Dataset identity and
full source content agree, including file hashes and publication state. JSON
formatting and known generated publication bindings do not create new scientific
variants. Sources retains every exact source link and original body.


### Dataset families and subgroups

A Task is the top-level browsing family. Reuse its exact reference; do not create
one Task per variant. A Dataset may declare `metadata.research.organization`:
`{"path":[{"id":"cohort","label":"Cohort"}]}`. With multiple linked Tasks,
include an exact `task_ref` to select the browsing family. This must be an existing
Task context; it does not create or link a Task. For new outputs, it must match
the accepted Job `scenario_ref` pin; finalization rejects a different Task. Paths support up to 12 subgroup
levels and stable local IDs. Reuse an existing path ID and label for the same group.

For an existing exact Dataset, preview `results.organize_dataset` with `target`,
`task_ref`, `path` and `reason`. Apply with `dry_run:false`, the returned
`expected_digest` and a stable `key`. The Task must already be linked. This additive
annotation retains previous assignments and source hashes. Changed sources require
a fresh preview. An empty path places the Dataset directly under its Task.

Subgroups describe shared collection context; they are not parent/child claims.
Use `lineage` or `results.link_dataset` only for evidenced data derivation. The
Dashboard nests loaded descendants across multiple generations within a subgroup;
multi-parent cross-links remain in detail. A parent outside the selected subgroup,
page or filter remains an exact cross-link rather than a duplicated Dataset.
Never sum overlapping child counts into a family total. Family counts count logical
Dataset entries after exact mirror grouping, not unique episodes.
