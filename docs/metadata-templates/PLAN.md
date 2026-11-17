# Output metadata templates and robot-first names

Status: implemented. Installation rollout is tracked separately. This document
records the design, impact review and acceptance boundaries. The generated native
reference describes the candidate contract, not the version of an installed host.

The [research reporting contract specification](REPORTING-CONTRACT.md)
extends this structure with pre-execution reporting declarations, semantic metric
and protocol pins, and evidence-complete publication. Those gates are implemented separately from metadata validation in `reporting.rs`.
The reporting contract also defines output eligibility: training reports attach to Policies,
comparisons are derived views, and Dataset diversity follows the selected SDG
factors. Historical missing measurements stay explicit; future required reports
are frozen before execution and validated at publication.
Reporting templates provide reusable minimum coverage. Authors may register
evidence-backed exclusions or replacements before submission, and must create a
new contract revision when necessary analysis is absent. Freeze the resolved
coverage and evidence with the Job; keep missing measurements distinct from
justified exclusions. Reporting adaptation does not weaken the
implemented metadata-template schema inheritance rules.

## Problem and intended behavior

Outputs currently combines Profile-specific JSON Schema with a compatibility
extractor that recognizes several historical field names. Structural validation
does not establish field meaning. An `episodes` array can be mistaken for an
episode count, a robot description can be omitted, and different producers can
express the same measurement through incompatible structures. Repeated display
curation cannot repair this input boundary.

New scientific records use a selected, immutable metadata template. A template
binds a JSON Schema to bounded display fields, units and output kind. Templates
describe structure; producers and evaluators retain responsibility for measured
values and scientific acceptance. Existing accepted records remain immutable.

Output names use `<robot model> · <subject> · <variant> · <purpose>` with a
revision or selected step only when needed to distinguish siblings. Robot is
also a typed property available to callers. Task context, output identity,
file names and Job titles remain separate. Do not infer a robot from a Task name,
policy architecture, runtime, owner or a neighboring record.

## Scope and implementation order

1. Inventory all requested existing Dataset, Model and Experiment records,
   including historical mirrors and operational records. Read exact metadata and
   bounded registered JSON evidence; do not recursively inspect every host file.
2. Review this plan, the feature ownership map and the documentation contract.
3. Implement the registry, template validation and exact source-bound adoption.
4. Bind templates at Profile registration and freeze them in Job definitions;
   validate result publication without changing execution/recovery ownership.
5. Update SDK, import/reconciliation, Dashboard and agent guidance together.
6. Verify structural failures, compatibility, preservation and real browser
   behavior. Prepare and apply only independently verified existing-record
   corrections within the user's requested data scope.

Platform deployment remains a separate explicitly requested operation. An
operational correction must use a supported installed contract or a reviewed,
tested additive migration. Do not install new service code to bypass a deployment
hold. Unsupported corrections remain staged with their exact prerequisites.

## Data model

### Immutable templates

Use the existing `catalog_sources` authority with kind `metadata_template` and
an exact `{id, revision}` identity. This avoids a second ad-hoc schema database
and participates in common Item allocation, dirty journaling and search. Reads
select an exact revision or explicitly list current revisions; writes never
replace an accepted revision. Existing revisions and request receipts survive
restart and replay.

A template contains:

- Stable ID, revision, label, output kind, origin and canonical content SHA-256.
- A bounded, local JSON Schema for the canonical research object. Reject remote
  references, executable renderers and excessive schema depth or size.
- Display descriptors: JSON pointer, semantic field ID, label, section, format
  and optional unit. Counts and scalar measurements cannot point at objects or
  arrays of raw episode records. Table/list fields have explicit bounds.
- An optional exact base-template reference. Extension validation intersects the
  base contract; it cannot weaken a standard field or silently change its units.

Ship small built-in templates for Dataset, generic/policy/observer Model, evaluation and
diagnostic Experiment. Built-in describes platform origin, not ownership of
research data. Custom templates are ordinary shared registered resources.

### Canonical metadata

Keep the existing `metadata.research` envelope. Pin the selected template by
ID, revision and SHA-256; preserve the pin in accepted output provenance.
Common fields describe robot, environment and relevant software references.
Kind-specific sections describe collection/splits/observations/physics/annotation,
model architecture/conditioning/training/selection, or evaluation population,
protocol/evaluator/criteria/outcomes. Unknown, inapplicable, false and zero are
distinct states. A missing robot cannot be replaced with an invented model.
Only applicable standard modules are accepted for each output kind. Dataset
organization retains its existing Task/subgroup validation, independently of
scientific measurements and derivation. Typed input declarations retain RGB-D,
proprioception, camera calibration/pose and validity-mask distinctions.

Robot has an explicit model and recording status. A multiple-robot record must
declare its models rather than select one arbitrarily. The display-name prefix
comes only from an unambiguous recorded robot; other records retain their names
until resolved. Standard names retain the source spelling unless a recorded
manufacturer/model alias establishes equivalence.

Do not put operational prose, installation logs, arbitrary nested manifests or
complete episode arrays in compact facts. Original payloads remain in Sources
and registered files. Detailed extension fields require a registered template
descriptor rather than an implicit alias guessed by the UI.

### Existing records

Adoption is a separate annotation with exact source identity/digest, selected
template pin, normalized metadata, field evidence, reason and actor. Preview
before apply; require the preview source digest and stable request key. Retain
the previous annotation in the operation receipt, and invalidate only affected
catalog projections. Do not modify original manifests, historical bodies, Job
or Attempt rows, scientific revisions, Task links, file hashes or storage IDs.

Classify each item as compatible, safely convertible, conflicting, unrecorded,
inapplicable or non-scientific fixture. Keep source/mirror identities distinct;
equivalent visible records can share a proposed transformation but retain their
own source-bound receipts. Do not turn a template adoption into a new experiment
or a claim of measured acceptance.

## Block impact and dependencies

| Block | Implementation boundary | Required consumer review | Verification |
|---|---|---|---|
| results | Template registry and source-bound metadata adoption; result validation | API, producer SDK, reconciliation, catalog | Immutable revisions, replay, conflicts, kind mismatch, schema/display limits |
| authoring | Profile template references and Rust producer metadata helpers | Job freezing, Component registration, guides | Exact pin, missing fields, old frozen Profile preservation |
| execution/recovery | Publication uses frozen template definitions | Result failures and reindex | A metadata error never reruns successful computation or consumes a recovery round implicitly |
| catalog/items | `metadata_template` Item/search projection; canonical robot and bounded scientific facts | HTTP/MCP, Dashboard, indexes | Search IDs, source-qualified aliases, stale annotation rejection, mirror preservation |
| reconciliation | Explicit template binding for new typed imports and existing adoption | Item lineage, byte verification | Preserve old adapters/receipts; reject stale contract contexts; no inferred robot or invented origin |
| dashboard | Template browse/inspection/selection and shared list/detail rendering | Outputs, Profile combinations, search | Robot property, compact fields, no raw objects, both themes and narrow panes |
| onboarding/API | Exact action schemas, examples, returns/errors, cache revision | Loom-Use and old clients | HTTP/MCP parity, generated reference, schema-valid behavioral examples |
| installation | Templates/annotations remain persistent instance state | Update preservation, rollback, packaged binary | No host paths in source; independent identities; preserved revisions |
| Loom-Use | Find a comparable exact prior record before publishing; select or register a compatible template | Server results/authoring/reconciliation guides | Compact Skill routing, package and client contract tests |

Dependency path: template definition → Profile pin → frozen Job → producer
metadata → publication validation → canonical projection → list/detail/search.
Existing adoption enters at the canonical projection after exact-source and
evidence validation. Neither path grants execution, installation or access.

## API and compatibility

Provide discoverable list/get/register/revise template operations and a bounded
metadata validation/preview operation. Existing-record adoption requires exact
entity/historical selectors, expected source SHA, reason and idempotency key.
The generated reference is authoritative for final action and field names.

Newly registered or revised Profiles bind immutable template definitions and
select them per output/kind. Existing accepted Profiles and in-flight Jobs retain
their original validation contract. Compatibility is explicit: legacy records
can be read and adopted; old Profile pins are never silently rewritten or made
to appear template-validated. Guide authors must migrate an appropriate Profile
revision before claiming a new producer follows the template contract.

New import contracts declare template references. Existing import plans retain
their frozen adapter and byte evidence. An overall contract-context change can
require a fresh plan under existing reconciliation rules; this change does not
bypass that guard. A template update does not repin an accepted Profile, Job,
output or reconciliation plan.

## Agent workflow and documentation review

Before publishing a Dataset, Model or Experiment, search for the most comparable
existing record by output kind, robot, producer/purpose and protocol. Inspect its
exact metadata/template revision. Reuse a compatible registered template; create
a new revision or derived template when genuine field coverage is missing.
Record the reference used for format selection. Copy structure, not measurements,
robot claims, cohort sizes, success criteria or scientific acceptance.

Review all current instruction surfaces under `loom_use`, the native action
descriptions/examples, root and feature documentation, Rust authoring reference,
generated API reference, and Loom-Use's Skill/docs. The Skill remains a short
route to connected-server authority. Historical design is marked historical and
does not become another operating checklist. Keep host audit evidence private.

Update `features/catalog.json` ownership and `documentation-catalog.json` for
new sources/documents. Run feature impact/check, build/check generated reference
with the tested native binary, and run Loom-Use instruction/package tests.

## Acceptance matrix

- New templates/revisions retain exact hashes; replay cannot mutate a definition.
- Invalid schemas, external references, oversized fields and incompatible
  standard-field overrides fail without partial registration.
- Known robot, unknown robot, inapplicable robot and multiple robots remain
  distinguishable. Robot-first names do not replace Task context or identity.
- Scalar counts reject nested episode records; split counts preserve zero and
  distinguish unrecorded from inapplicable. No source alias can bypass types.
- Exact selected template is checked at Profile registration and publication;
  unknown pins, changed hashes, wrong output kinds and invalid metadata fail.
- Old frozen Jobs remain runnable under their accepted contract. Result repair
  preserves producer success and does not rerun training or generation.
- Existing adoption previews exact before/after data, rejects source/evidence
  changes, is idempotent and preserves originals and independent installation state.
- Dashboard list/detail/inspection use the same template facts; custom display
  fields are bounded, escaped and readable in both themes and narrow panes.
- HTTP/MCP discovery, Rust SDK, documented examples and installed/pinned client
  behavior agree. Missing tooling or deferred deployment is reported explicitly.
- Existing initial-Dashboard Job paging correction remains covered and unshipped
  until deployment is requested.

## Implementation review

The registry uses existing persistent tables; no new host directory, credentials
or mandatory external service is introduced. New Profile registration is the
producer enforcement boundary. Read compatibility for accepted legacy Profiles,
source bodies and v1/v2 captured facts remains explicit. Dataset/Model/Experiment
adoption is an operator declaration, never file verification or scientific
acceptance. Unconvertible values remain in original Sources.

API/MCP discovery, the Rust producer helper, typed v3 imports, Item/search
projection, Dashboard relay allowlists, Profile inspection, Loom-Use routing and
server guides were reviewed together. Template lists carry compact descriptors;
exact gets carry full definitions. Dashboard reads are bounded and paged, and
its read relay rejects registration/adoption writes. The deferred first-load Job
paging correction remains an independent regression case.

Verification covers immutable revisions, local schema-reference scoping, invalid
types, zero/unknown/inapplicable/multiple robot states, frozen Profile pins,
publication failures before indexing, source-bound adoption replay, original
execution preservation, curated-name consistency, HTTP/MCP parity and Firefox
light/dark layouts at wide/narrow sizes. Operational release/image/reboot
acceptance remains outside this source-only change.
