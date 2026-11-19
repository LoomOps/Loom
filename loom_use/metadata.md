# Select and publish output metadata

Before registering or publishing an entry, select its identity and relationships using [catalog request walkthroughs](catalog-cases.md).

Use registered metadata templates for new Dataset, Model and Experiment producers.
Templates validate structure and display fields; they do not establish scientific
acceptance. Read `results.metadata_templates` and `results.get_metadata_template`
through the connected server's exact action schemas. Also follow
[reporting](reporting.md) when creating scientific producers: choose or author
analysis coverage, justify exclusions, implement reporters and freeze the contract
before submitting. Metadata validation alone does not satisfy reporting gates.

Before authoring metadata, search existing outputs for the most comparable kind,
robot, purpose and protocol. Inspect that exact record and template revision.
Reuse its format when appropriate. Copy structure, not measurements, cohort sizes,
robot claims, success criteria or acceptance. Keep the exact comparable reference
as `metadata.format_reference` or the adoption request's `format_reference`.

## Select a format

1. List templates for the output kind and read the selected revision. Its
   `{id, revision, sha256}` is the template reference.
2. Reuse the template if it covers the required fields. Otherwise register a
   derived template with an exact `base_ref`, extension schema and typed display
   descriptors. Revisions cannot weaken the base or replace its standard fields.
3. Set the Profile's `metadata_templates` selector map. Keys are `dataset`,
   `model`, `experiment` or `output:<local-id>`; values are exact references.
   Newly registered Profiles freeze definitions in `metadata_template_pins`.
   Unspecified kinds use the platform's generic Dataset/Model/evaluation formats.
4. Validate a draft with `results.validate_metadata`. Read the returned field
   errors rather than changing the template to disguise a wrongly typed value.

Custom display fields live under `research.extensions`. Every extension needs a
registered descriptor. Large manifests and episode records belong in registered
files and Sources, not in an episode count or a compact property.
Dataset `research.organization` retains the existing Task/subgroup contract; it
does not describe a measurement or replace derivation links.

## Canonical values

Use `metadata.research`. Robot is an object with `status` and, when recorded,
`model`. Supported statuses are `recorded`, `unrecorded`, `not_applicable` and
`multiple`; multiple robots use `models`. Preserve source-supported model names.
Do not infer a robot from a Task, policy architecture, runtime or nearby record.

Dataset `collection` declares its recording status and a count/unit when recorded.
Splits retain explicit zero, `unrecorded` and `not_applicable`. Model metadata
separates policy/observer role, architecture, conditioning and training selection.
Evaluation metadata separates offline/closed-loop scope, Sim/Real environment,
protocol/evaluator, population and success criteria. Use the selected schema for
the exact fields and units. New fields require compatible template coverage.

Use `collection.split: {"train":90,"validation":6,"test":"unrecorded"}`.
For legacy `{id,count,frames}` split rows or nested count objects, copy the
recorded count into this map and retain frames/member records in Sources.
Do not replace a missing count with zero, infer a split from the label, or flatten
train/validation/test into unrelated extension fields. After adoption, verify the
actual Split value in both the exact Dashboard entity and its list page; a valid
template pin alone does not prove that recorded fields survived conversion.

With Rust, register the output, call
`producer.set_entity_metadata(local_id, template_ref, research)`, set its readable
label, then finish. The server validates against the frozen Profile and prefixes
an unambiguous recorded robot model. External typed imports use `loom-asset/v3`
with `definition.metadata.template_ref` and `definition.metadata.research`.

## Existing records and compatibility

Use `results.adopt_metadata` to preview one exact native/historical source. Apply
with the returned `expected_digest`, `dry_run:false`, stable key and reason.
Keep field evidence and the comparable format reference. This is an additive
operator declaration, explicitly not a new verification or scientific result.
It preserves original manifests/bodies, scientific revisions, files and execution.
Its receipt retains the previous annotation for an explicit later correction.
Current result/Dashboard entity reads expose `metadata_provenance` with the
operator declaration, exact source digest, reason, evidence and format reference.

Legacy Profiles/Jobs and old import adapters retain their frozen contracts. They
remain marked legacy until explicitly normalized; merely viewing a record does
not validate or migrate it. Register a compatible new Profile revision before
claiming a producer uses the template contract. Existing reconciliation plans
retain their source adapter and remain subject to their original context checks.

Registry Archive/Restore and deprecation use `components.inspect_lifecycle` followed
by `components.set_lifecycle`. See the Components guide for exact target selectors,
authorization and explicit batch selection. These overlays preserve sealed pins and
files; they are not execution or scientific acceptance gates. Discovery accepts
`include_archived` / `lifecycle_state`; exact reads remain available.
