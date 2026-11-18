# Research reporting contracts and detailed output views

Status: implementation specification. Native development source implements the
registry, coverage adaptations, admission/publication checks and bounded report
views. Consult connected-server discovery for the installed revision. Existing
records are not retroactively made complete.

## Problem

Typed metadata alone allows a producer to omit the reports needed to inspect its
result. Free-text result labels, protocol names and scope descriptions can hide
different populations, controllers or success predicates. Registered report files
already contain measurements that the compact catalog projection does not expose.

Treat output inspection as a contract chosen before execution. Separate:

1. A metadata schema: names, types, units and applicability.
2. Metric definitions and evaluation protocols: what is measured and how.
3. A report contract: required reports, sampling, populations and exact inputs.
4. A display projection and View template: bounded inspection of accepted reports.

No View setting can change measurement semantics, waive a required report or turn
missing data into a zero. Presentation revisions do not revise scientific results.

## Reusable minimum coverage and authoring

A report contract is a reusable coverage template, not a closed list of everything
Loom permits a producer to analyze. Its default requirements provide the starting
minimum for the declared activity. The agent using Loom-Use must assess the actual
Job's questions, inputs, outputs and intended claims before selecting a contract.
The final frozen contract is that Job's effective coverage, including justified
exclusions and any added requirements.

Before submission, the author follows this sequence:

1. Discover comparable outputs and existing contract revisions; reuse a suitable
   contract and reference the comparable structure without copying its results.
2. Compare its coverage with the planned analysis. Identify applicable requirements,
   requirements that do not fit, and necessary analysis that the template omits.
3. For a requirement to exclude or replace, register an evidence-backed adaptation
   with the exact source contract/requirement, reason, applicability evidence,
   resulting scope/analysis limitation and any replacement requirement.
4. If necessary coverage is absent, register a new contract or derived revision
   with the new metric/report schemas, units, populations, applicability, evidence
   roles and supported display descriptors. Add reporter/evaluator capabilities
   before selecting it. The absence of a built-in field is not a reason to omit
   necessary analysis or place it in an untyped text field.
5. Validate and register the immutable contract; select it in the Profile or Job
   binding supported by the eventual API. Preview the resolved requirements and
   exclusions, then freeze their exact pins at submission.

Ordinary contract authors can register these adaptations within their existing
authoring scope. This does not require a platform release, administrator-authored
template or an extra approval hierarchy. Registration validates the contract and
evidence bindings; Job admission separately checks reporter support and exact
input compatibility. New display fields reuse typed UI primitives; executable
renderers and arbitrary scripts are not part of a contract.

An exclusion is a coverage decision, not a fabricated measurement. Its record
contains `source_contract_ref`, `requirement_id`, `reason`, exact `evidence` refs
and field pointers, `analysis_impact`, and an optional `replacement_ref`. Narrative
alone is insufficient evidence. Missing an implementation or disliking a measured
result does not establish that a requirement is inapplicable. A deliberately
narrower analysis may use a new contract when its evidence, scope and remaining
claims are explicit and consistent; it cannot retain the broader coverage claim.

Keep the effective plan's `included`, `excluded` and `replaced` requirement
dispositions separate from report-value states. A justified exclusion is visible
in contract details with its reason and evidence, not displayed as a missing
measurement. Uncollected applicable values remain Missing or Not measured. The
validator rejects changed/missing evidence, contradictory applicability, unknown
definitions and replacement cycles. It cannot prove the scientific adequacy of
every author-supplied rationale; the retained provenance makes that decision
inspectable and revisable prospectively.

For example, a Policy with a verified language-free input schema may exclude the
instruction report. A native language-conditioned Policy cannot do so. An imported
checkpoint Job may exclude optimizer telemetry it did not produce, documenting
the external history limit; a native training Job cannot claim the same scope.
An SDG study needing contact-material coverage absent from the selected template
registers the factor/report definitions and a new contract before running.

Identity, evidence integrity, output-kind consistency, units/population semantics
and immutable accepted execution are platform invariants, not analytical checklist
entries an author can waive. Before execution, analytical coverage can be adapted
with evidence. After submission, it cannot be weakened retroactively to pass
publication; a later contract revision applies to a later acceptance, with links
to the earlier evidence and no invented rerun.

Report-contract adaptation is distinct from metadata-template
`base_ref` extension, which intersects schemas and cannot weaken inherited fields.
Do not implement exclusions by silently changing that existing API. The
report registry resolves an adaptation into an independently validated effective
contract and retains its source-contract lineage and exclusion evidence.

## Output eligibility

The primary Outputs path is Dataset -> trained Policy -> Policy evaluation.
Derived Datasets retain exact parent revisions and transformations; creating a
new Dataset does not create a new Task or break its parent chain.

| Material | Destination | Independent Experiment? |
|---|---|---|
| SDG collection or transformed Dataset | Dataset revision with membership and generation evidence | No |
| Trained Policy checkpoint | Policy revision with Dataset/checkpoint pins | No |
| Training/validation loss, selection and optimization report | Attached Policy reports, accessible through Training | No |
| One evaluation execution under a frozen protocol | Experiment with exact Policy, cohort and per-case evidence | Yes |
| Cross-Policy or cross-run comparison | Derived comparison view over exact accepted Experiment revisions | No |
| Observer weights, runtime setup, smoke or platform acceptance | Owning Tool/Component or operational Job evidence | No |

Independent offline evaluation is allowed when it has its own frozen test protocol,
Policy, cohort and case/measurement evidence. A training report's validation or
checkpoint-selection table is not an independent evaluation merely because it
contains a held-out loss. A multi-Policy comparison refers to its constituent
evaluations; it must not register an additional scientific run for their union.

Admission validates the activity's declared output roles, not the producer's
chosen title. Publication rejects `training_report` or `comparison` submitted as
an Experiment with `output_kind_mismatch` and the supported attachment/view route.
This is distinct from an evaluation's legitimate aggregation of cases within one
protocol execution. Historical misclassified entries keep their IDs and evidence;
an audited presentation/attachment mapping removes them from the primary results
view without deleting their execution or inventing new evaluations.

## Recorded, missing and inapplicable values

Reports use typed states: `recorded` includes a value and evidence; `missing` means
expected evidence is absent; `not_measured` means a known applicable measurement
was not performed; `not_applicable` follows resolved applicability or an accepted,
evidence-backed exclusion and carries its reason. Zero is an actual measured value,
not a missing-state code.
The UI renders Missing, Not measured and Not applicable respectively, including
on legacy records. It never infers a scientific value from a display name.

Legacy incompleteness does not block reading. For future native Jobs, the frozen
report contract determines whether missing evidence prevents publication. A
producer cannot replace a required measured value with `missing`, choose an
arbitrary inapplicability reason, or remove a report after execution to pass the
gate. Interrupted/failed work retains its evidence without a completed-result
claim. A validated pre-submission exclusion is not an arbitrary inapplicability
claim or a missing required report. Optional metrics do not become mandatory
merely because the UI can render them; changing scientific coverage requires a
new effective contract revision.
These are report-value states; existing metadata schema fields retain
their documented `unrecorded` and other status enums until explicitly versioned.

## Shared identities

Templates, metric definitions and protocols have immutable `{id, revision,
sha256}` references. A Profile selects report contracts and declares reporter
capabilities. Job submission freezes the selected definitions and exact source
inputs; accepted Jobs retain those definitions through execution and publication.
The frozen bundle includes the resolved coverage digest and exact adaptation
evidence, not merely the default template's ID. Future Jobs cannot silently pick
up a changed template or exclusion under an existing pin.

Dataset inputs include exact revisions, selected members/splits and membership
digests. Policy inputs include an exact policy revision, selected checkpoint and
weights/config/processor digests. Runtime, evaluator Tool/Component and code pins
remain separate references. A string naming a policy or dataset is insufficient.

For current interfaces, preserve internal `model` identities while using Policies
for the policy-facing UI. An Observer is not reclassified as a Policy: its learned
weights stay linked to the owning Tool and retain their original identities.

## Metric definitions

A registered metric definition records:

- Semantic ID/revision, quantity, label, value type, canonical unit and direction.
- Population grain: frame, valid target, joint vector, episode, condition or run.
- Exact aggregation/reduction, masking and weighting rules; conditioning and
  denominator definitions for rates.
- Applicable report/activity and dimensions: split, phase, joint, checkpoint,
  seed, cohort or environment.
- Evidence requirements and calculation/evaluator version.

Normalized denoising loss, physical-unit action error and rollout success are
different metric families. Neither names nor numerical similarity make them
interchangeable. A result chooses its primary metric through its frozen protocol;
the UI does not choose the largest number or accept a new free-text definition.

An All metrics inspector exposes every recorded supported metric with its unit,
population, dimensions, definition and evidence. Overview highlights are a small
stable projection of that registry, not another independently edited summary.

## Policy report

### Instructions

Language-conditioned Policies require an instruction report. It includes exact
source text/templates, revision/digest, language, encoding/tokenizer, original
field, phase/episode/time applicability and the rule mapping observations to an
instruction. Parameterized instructions retain variable names/types/units and a
hash-bound per-example assignment manifest. Inspect resolved examples without
reconstructing text from a vague label such as "Phase".

Policy Overview shows the actual representative instruction text, rather than
only the word Language or a count. Instructions opens the complete texts, mapping,
source pins and coverage. Dataset language annotations, generator prompts and
Policy-consumed instructions have separate roles; they are not interchangeable.

Declare unconditional operation through the contract's supported `none` mode.
It cannot be selected when the accepted Policy input schema consumes language.
An SDG generator prompt and a stored Dataset language annotation are distinct.

### Training and validation

For a training activity, declare required metric series, optimizer-step axis,
logging/evaluation schedule, resume segments, populations and selection rule before
the Job starts. Reports preserve raw points and null gaps; the UI must not invent
a train curve from validation points, smooth away failures or silently interpolate
missing checkpoints. Explicitly report skipped/overflowed optimizer updates.

Common supervised/IL coverage includes train objective, held-out objective,
objective components, learning rate, gradient norm, consumed frames/episodes,
optimizer updates, elapsed time and checkpoint selection. Physical-unit action
MAE/RMSE are resolved by phase, joint and action horizon where the accepted action
schema and evaluator support them. Denoising/flow likelihood objectives keep their
own definitions. RL return, actor/critic losses, entropy or KL apply only to an RL
recipe that declares them; unrelated metrics are not fabricated to fill a form.

The inspector provides step-range and phase selection, per-checkpoint points,
per-joint error tables, valid-target counts and the exact evaluation/normalization
definition. Validation can choose a checkpoint; held-out test measurements must
not be reused to select it. Distinguish requested points from observed points.

For Policy import/export, require the weights/config/interface/instruction and
source-training references. An explicitly typed external/unavailable training
history is a different accepted contract, not a caller-controlled bypass for a
native training Job that promised a learning report.

## Dataset report

Require the schema, membership, parent/transformation references, split definition
and a statistics report for the declared Dataset type. The report covers:

- Counts and units, split/task/phase/source/condition coverage and the chosen
  independence unit. Episode length/duration is an optional quality diagnostic;
  it is not the primary SDG diversity view or a required proxy for spatial coverage.
- Observation/action names, joint order, shapes, dtypes, units, masks, rates,
  timestamps, camera configuration and calibration when applicable.
- Per-channel ranges, moments/quantiles, missing/invalid/NaN/clipped values and
  sampling intervals when those signals are part of the Dataset contract.
- Duplicate and split-overlap checks at each declared level: frame, episode,
  source demonstration, condition or identity. Zero overlapping episode IDs does
  not imply independent source demonstrations.
- The SDG factors the generator intends to expand: object/target position and
  orientation, relative geometry, robot initial state, camera, physics or other
  explicitly selected factors. Requested settings and readback remain distinct.
- Stored instruction/annotation coverage and provenance when present.

Statistics declare the measured subset, total population, exclusions, units,
bin boundaries/counts, quantile method and source digest. Indexed raw data or
sealed report files hold the full report; catalog DTOs carry bounded summaries
and references. Histogram bins never imply a distribution that was not measured.

Dataset Overview includes collection checks and data lineage. Distribution and
Episodes are available when the report has their coverage or membership data.
Policy Overview includes the consumed instructions and exact accepted inputs.
Training includes the selected checkpoint, validation series and offline metrics.

### SDG factor coverage

Before generation, select versioned factor definitions and record each factor's
target entity, coordinate frame/reference pose, unit, dimensions, treatment
(`varied`, `fixed`, `not_measured`, `not_applicable`), sampling rule, bounds/levels,
constraints and seeds. This report is selected by the SDG contract, not inferred
from generic metadata names such as `position`. State which factors are controlled
and which nuisance factors are merely observed; fixed yaw is not yaw diversity.

Every generated member maps to its condition ID and exact generation attempt,
requested factor values, reset/readback phase, measured values and acceptance.
Keep planned, attempted, accepted, failed and excluded populations separate. Report
coverage against the planned factor space, rejected regions, and counts per
split/condition. Preserve constraints and correlations: independent marginal
histograms cannot establish coverage of a joint position/orientation space.

Use XY scatter or spatial bins for positions, angular/level coverage for rotations,
per-level counts for discrete factors and paired coverage for explicitly planned
factor interactions. Use shared axes/units across splits, show the source-demo
support, and distinguish condition-held-out from source-held-out evaluation.
No factor is implied to be randomized just because an external benchmark varies it.

A reset inventory from another attempt is supporting evidence until exact member
revision/attempt bindings are verified. Shared condition IDs alone do not establish
identical assets, physics, camera, controller or initial state. Full raw samples
stay in sealed/indexed files; the catalog carries bounded factor summaries and
report references. No historical host coordinates or measured datasets belong in
public built-in templates.

### Report roles

These are report roles, not action names. Exact current payloads and actions are
described in the [reporting guide](../../loom_use/reporting.md).

| Role | Required contents | View |
|---|---|---|
| `factor_manifest` | Definition pins, entity/frame/units, sampling plan, bounds/constraints, fixed controls, condition IDs and split membership | Dataset Overview and factor selector |
| `condition_observations` | Condition/member/attempt pins, requested values, reset phase and observed values, acceptance and exclusions | Planned versus observed coverage |
| `instruction_manifest` | Exact text/templates, variables, assignments and source pins | Policy Overview and Instructions |
| `learning_series` | Metric definition pins, step/phase/split axes, populations, selection and interruptions | Policy Learning/Metrics |
| `evaluation_cases` | Exact Policy/cohort/protocol, case/seed/attempt, factor assignments, validity, outcome and failure/measurements | Experiment Cases and conditional Metrics |

The reader resolves these roles from the exact output revision and pages indexed
reports; it does not search arbitrary filenames or require full raw trajectories
inside an MCP response. The writer first resolves the selected contract and
declares reporter support. Shared core validation then freezes that bundle at
submission and validates these report roles at publication for HTTP and MCP alike.
Repair adds a traceable corrected report revision; it cannot repin the accepted
protocol or silently change the selected measurement populations.

## Experiment report

An evaluation protocol fixes all of the following before execution:

- Exact Policy checkpoint, Dataset/cohort membership and split, scenario/context,
  reset/seed policy and evaluator/runtime/code revisions.
- Typed mode (`offline` or `closed_loop`), environment (`sim` or `real`) and
  controller composition (`policy`, `hybrid` or `planner`). These describe
  different axes and cannot be replaced by a generic word such as "research".
- Phase/controller ownership, what each controller observes, GT access for policy,
  planner and scoring, and whether the Policy remains active during hold.
- Primary outcome definition, success predicates with units/tolerances/windows,
  secondary/diagnostic/conditional metrics and failure/termination taxonomy.
- Planned cases, sampling/retry policy, denominator/exclusion rules and the
  distinction between infrastructure retries and scientific repeats.
- The selected analysis factors, reference condition, intervention levels and
  planned cross-factor tests; exact instruction text/assignment is pinned alongside
  camera configuration, robot state, physics and other required nuisance controls.

Case reports preserve case/seed/attempt IDs, executed input pins, timestamps,
validity, outcome, failure stage and measured values. Aggregates must reconcile
with those rows. Planned = unexecuted + executed categories under the declared
protocol; valid/invalid/infra/pending classifications must be exclusive as defined.
Measured metric populations can be smaller than valid-case populations and must
report their own count. An unobserved failed-stage measurement is not a zero.

For conditional success, a zero denominator is `not_applicable`, not a 0% result.
Per-phase success does not replace whole-task success. Planner-assisted hold is
not evidence of Policy-only hold. Comparative views validate protocol/predicate,
population, inputs and scope compatibility before calculating a comparison.

Evaluation Overview includes the protocol and exact Dataset and Policy inputs.
Results shows verified aggregates and measurements. Cases opens the matching
case and seed. The list's Result, Protocol and Scope fields come from the
registered definition and frozen execution.

### Performance factors and robustness

Dataset factor IDs and evaluation condition IDs share semantic definitions, but
retain separate memberships. Evaluation views expose training support versus
evaluated ranges/levels, in-distribution versus held-out status where justified,
and per-factor/level success and failure-stage counts. A protocol can vary object
layout, camera pose/FOV, robot initial state, language, lighting, background or
sensor noise. These are selectable analysis dimensions, not seven mandatory tests
for every task. Physics/contact/controller factors are added when the task's
protocol selects them. No empty report is presented as a robustness evaluation.

For each selected slice, retain successes, valid/planned counts, excluded/infra
counts, perturbation level and baseline reference. Show a performance difference
only when denominator rules, checkpoint, cohort pairing and protocol controls
support that comparison. Preserve pairwise/multiple-factor interventions; a suite
average cannot replace conditional results. Instruction paraphrases, blank text
and changed goals have distinct semantics and their own ground-truth task labels.

Expose failure stage, goal distance/orientation errors, stability/drift and control
ownership when measured by the accepted protocol. Policy error, planner assistance
and whole-system success remain distinct. Missing factor labels prevent the
corresponding conditional calculation; they do not produce an "Other = 0" bin.

## Two enforcement boundaries

### Admission

`jobs.submit` and every workflow submission share native core enforcement. Before
queue insertion, resolve the Profile's report contract and freeze report schemas,
metrics, protocol, input revisions, required file roles and reporter schedule.
Reject missing declarations, incompatible input kinds/revisions, unsupported
required metrics, absent evaluator/reporter capabilities and contradictory scope.
Resolve and validate evidence-backed exclusions and replacement requirements
before these checks. A default template mismatch routes to contract adaptation
or new-contract authoring; it does not require pretending that the Job fits.
Future measurements do not exist yet: this stage validates the plan and capability,
not fabricated result values.

Applicability follows the registered activity and validated effective contract,
not a request's presentation label or an arbitrary `required:false`.
Scientific training/SDG/evaluation cannot evade
the contract by declaring every result as a generic Artifact. Child executions
may contribute reports to one parent Job; they do not need unrelated dashboard
records or independent scientific Jobs.

### Publication

Check required report roles and source-file hashes, local report schemas,
instruction coverage, series/phase/split/joint populations, unit/metric definitions,
input/checkpoint bindings, complete schedules or explicit interruptions, case
accounting and aggregate recalculation. Failures return structured field/report
errors before scientific registration and before `artifact_ready` becomes true.
Evidence files and their index remain available for repair and diagnosis.
Only included/replacement requirements in the frozen effective contract require
reports; exclusions retain their evidence and do not demand a fake placeholder
file. Publication cannot create new exclusions from the observed outcome.

A successful computation with an incomplete report remains a publication error;
it is not automatically recomputed. Repair/reindex reuses the frozen contract.
It cannot weaken the promised requirements or overwrite the original execution.
Hash/schema/accounting validation establishes structural and byte evidence, not
the scientific truth of every sensor or simulator measurement.

## Frozen expected values

Profiles may declare `reporting.expectations`, mapping contract selectors and
requirement IDs to pointer bindings. A binding uses `{pointer, value}` or
`{pointer, context_pointer}`; context is restricted to accepted params/Profile.
Native training requires an explicit schedule binding for required learning
reports. Admission resolves referenced context, and publication compares actual
report data with that frozen plan. A report cannot shorten its schedule or change
its promised target count after execution merely by changing its own declaration.
The scientific author chooses the applicable targets; schema/hash validation does
not establish that every chosen target or measurement is scientifically adequate.

## Compatibility and rollout

Previously accepted Jobs finish under their original pins; existing records remain
readable and clearly distinguish legacy or incomplete reporting. No missing loss
curve, prompt, distribution or case record is synthesized during migration.

Before enabling strict new submissions, inventory active Profiles and create
compatible explicit revisions with reporters. New scientific submissions through
legacy Profiles are rejected after cutover; existing in-flight Jobs are preserved.
The operator update plan reports unready Profiles before cutover. There is no
implicit always-on "legacy" bypass or destructive rewrite of runtime extensions.

## Implementation blocks

| Block | Required change | Regression boundary |
|---|---|---|
| results/contracts | Registered semantic metrics, protocols, authored report contracts and evidence-backed adaptations | Immutable revisions, resolved coverage, exclusion lineage, units/populations, missing evidence |
| extensions/Profile | Reporter capabilities, exact effective-contract selections and activity applicability | Validated exclusions, no unvalidated waiver; preserved accepted Profile revisions |
| jobs/workflows | One admission validator before queue insertion | HTTP/MCP parity, atomic rejection, child/parent report aggregation |
| authoring/SDK | Instruction manifests, named metrics/series, case and statistics reports | Actual files/points, resume axes, masks/units, Rust-first producer helpers |
| execution/recovery | Keep computation outcome separate from report acceptance | No accidental retraining after a report/index error |
| results/publication | Byte/schema/population validation and transactional publication | Missing reports, swapped checkpoints, aggregate/case mismatch, interrupted runs |
| catalog/index | Bounded report descriptors with exact source and metric references | Large series/files stay paged; no raw-array scalar or free-text inference |
| Dashboard | Output-specific Overview, Distribution, Episodes, Training, Results and Cases views | Shared definitions, excluded vs missing vs zero, exact links, theme/narrow layouts |
| Loom-Use/docs | Assess coverage; reuse, adapt or author/register a contract before creating/submitting software or Jobs | No omitted necessary analysis because a built-in field is absent; no claim of hard enforcement from Skill prose alone |
| installation | Profile readiness preflight and coordinated strict cutover | Preserve state, credentials, historical records and in-flight Job pins |

## Acceptance examples

- Accept a pre-submission exclusion with bound applicability evidence; retain its
  reason/scope and require only the resolved effective contract's report roles.
- Reject an unsupported narrative-only exclusion, contradictory input schema or
  changed evidence digest. No extra administrator approval is required.
- Register/select a new analysis contract with a new factor/metric and reporter
  when existing templates lack necessary coverage; preserve its display schema.
- Reject retrospective exclusions and changed coverage under an accepted pin.
- Keep validated exclusions out of Missing counts; keep genuinely uncollected
  applicable measurements in Missing/Not measured counts.
- Reject a new training plan without a reporter for a mandatory validation metric.
- Reject training reports and multi-Policy comparisons declared as Experiments;
  retain valid within-run evaluation case aggregates and independent offline tests.
- Reject SDG publication missing a required planned-to-accepted condition binding;
  do not substitute episode lengths for promised object-position coverage.
- Reject a caller's `not_applicable` claim for a protocol-required factor report.
- Reject a language-conditioned Policy report with no exact instruction payload.
- Reject normalized MSE declared as a physical-unit RMSE.
- Reject a closed-loop result without a pinned evaluator, cohort or Policy.
- Reject a renamed primary metric/predicate after Job submission.
- Reject an aggregate 94/96 backed by 93 successful and 3 failed valid cases.
- Preserve 94 measured arrival errors and two missing pre-arrival measurements;
  do not turn that into 96 error measurements with two zeros.
- Reject overlapping splits when the protocol requires source-disjoint splits;
  retain legitimate shared-source SDG when the protocol declares condition holdout.
- Reject all-null/empty required curves and an unrelated Job's training report.
- Retain interrupted run evidence without advertising a completed reporting claim.
- Open exact Dataset/Policy revisions and filter measured metrics by phase, joint
  and step range without mixing populations or silently interpolating points.

## Reference basis

[LIBERO-Plus](https://sylvestf.github.io/LIBERO-plus/) analyzes controlled shifts
in layout, cameras, robot state, language, lighting, backgrounds and sensor noise,
including combined perturbations. Its factor-level and instruction-following
analyses motivate linking Dataset coverage to evaluation slices. Loom does not
claim those benchmark results for a local Policy or prescribe the benchmark's
complete factor suite for unrelated tasks.

The proposed measurement families follow the actual producer/evaluator recipe.
[LeRobot training](https://github.com/huggingface/lerobot/blob/main/src/lerobot/scripts/lerobot_train.py)
records training objective and optimization diagnostics; its
[training configuration](https://github.com/huggingface/lerobot/blob/main/src/lerobot/configs/train.py)
distinguishes held-out loss evaluation from rollout evaluation.
[LeRobot benchmark evaluation](https://huggingface.co/docs/lerobot/main/adding_benchmarks)
retains per-episode outcomes and success counts. Statistical uncertainty is a
separate metric/view requirement rather than an automatically added UI claim.
[robomimic Dataset structure](https://robomimic.github.io/docs/datasets/overview.html)
provides observation/action schema and split/filter inspection examples.
[Datasheets for Datasets](https://arxiv.org/abs/1803.09010) motivates explicit
composition, collection and intended-use documentation. These are references for
the design, not a claim that a specific host uses their current versions.

## Action dimensions

Robot DoF counts independent mechanical joints; controlled joint DoF describes the
Policy-controlled subset. Action dimension counts ordered outputs and may include
gripper, Cartesian or latent channels. No equality is assumed. Record channel
names, indices, representation, joint identity and units. Missing mechanical DoF
stays Missing. Per-channel MSE/MAE/RMSE retain phase/split populations and
normalization: MSE uses squared units, MAE/RMSE original units. Never pool mixed
physical units into a single physical MSE.
