# Plan and publish analysis reports

Select reporting coverage before submitting a scientific Job. Metadata templates
validate catalog fields; report contracts specify the analysis evidence to produce.
Read the connected server's exact action schemas for the following operations.

## Reuse or author coverage

1. Search comparable outputs and inspect their exact reporting references. Reuse
   structure, never measurements, cohort sizes or acceptance claims.
2. Read `results.report_contracts` and `results.get_report_contract`. Built-in
   templates cover Dataset membership, SDG, Policy training/import and evaluation.
3. Compare required analysis with the template. Reuse its exact
   `{id, revision, sha256}` if suitable. When necessary coverage is absent, register
   metric/factor/protocol definitions with `results.register_report_definition`,
   then register a new contract with `results.register_report_contract`.
4. To adapt coverage, set `base_ref`. Add requirements under new IDs. Each excluded
   or replaced inherited requirement needs its ID, disposition, reason, analysis
   impact, exact source evidence and a Job applicability predicate. A replacement
   also names a newly added requirement.
5. Preview with `results.validate_report_contract`. Register immutable revisions
   with the expected revision and an idempotent request key.

Root contracts retain their activity's minimum report roles. To remove an
inapplicable role, derive with evidence instead of creating an empty root. Evidence
identifies an exact native/historical record or registered software revision,
source SHA-256, JSON pointer and actual value. Obtain the complete evidence object
with `results.get_report_evidence({target, pointer})`; select a subtree below 64 KiB. `when` compares a `/params/...` or `/profile/...` pointer with
`equals`; admission checks the real Job. Registration verifies source bindings,
not the scientific sufficiency of every rationale. Ordinary authors can register
within submit scope; no administrator hierarchy or platform release is required.

Returned `effective_requirements`, `definition_pins`, `resolved_adaptations` and
`lineage` and `excluded_requirements` are compiled fields, not author inputs. For a new root, copy only authored
fields. For derivation, use `base_ref`, new requirements and adaptations. Existing
metadata-template schema intersection rules remain separate.

## Bind the Profile

Set `reporting.contracts` using `dataset`, `model`, `experiment` or
`output:<local-id>` selectors. Set implemented roles in `reporting.capabilities`.
The server supplies `reporting_pins` and `reporting_policy`.

Policy Profiles declare `reporting.conditioning` as `language` or `none`.
`reporting.input_kinds` maps declared input names to `dataset` or `model`. Training
requires a Dataset input; evaluation requires Dataset and Policy inputs. Use exact
artifact revisions or upstream scientific outputs for those identities.

The built-in evaluation contract is a coverage template. Register a task-specific
protocol with a pinned primary metric, offline/closed-loop, Sim/Real and controller
scope. Declare `scope` with evaluator (`{kind: producer}` or exact Tool alias/ref),
phase/controller ownership, policy/planner/scoring GT access, whether the Policy
is active during hold, physics/control rates (or null), and the goal frame.
The built-in case template uses valid-case success rate as its primary metric;
continuous diagnostic metrics use additional registered report requirements. Its predicate has `all` rules with measurement pointers, `lte`, `gte` or
`between`, thresholds and units. Register a contract based on the evaluation
structure with that protocol pin in its case requirement's `definitions`, then
bind the Profile. Case success must agree with the frozen predicate.

Use `reporting.expectations` to freeze values or bind report fields to Job context.
It maps a contract selector to requirement IDs and arrays of
`{pointer, value}` or `{pointer, context_pointer}`. For example, bind membership
`/count` to `/params/target_count`. Native training requires a `/schedule` binding
for each required learning report; declare the actual logging/evaluation steps
before execution. Context pointers resolve only inside params or the frozen
Profile. Publication compares report data with those accepted values.

Submission freezes effective coverage and evidence. New scientific submissions
through legacy Profiles return `report_profile_legacy`; register a reporting-ready
Profile revision. Accepted legacy execution retains its original contract.

## Produce

Use Rust `Producer::report(output_id, requirement_id, data)` after registering the
pending output. It writes JSON, binds SHA-256 and registers the output component.
The request includes reporting pins and exact `report_inputs`; the helper fills an
existing `data.inputs` field from those accepted inputs. Python integrations may
write the same public envelope.

Reports contain `schema: loom-report/v1`, exact `contract_ref`, `requirement_id`,
`producer: {job, attempt, profile_sha256}` and schema-valid `data`. Output
`reporting` holds the contract reference and `reports` entries
`{requirement_id, component, sha256}`. Each report is bounded to 8 MiB. Declare
additional report requirements/shards for larger analysis.

- Dataset membership IDs, splits, sources and counts must reconcile.
- SDG target conditions and generation readbacks remain distinct. Account for
  every condition and exact accepted member/attempt. Episode length does not
  substitute for object-position coverage.
- Instructions record exact consumed text, assignments and coverage. Generator
  prompts and Dataset annotations are different roles.
- Learning retains measured step/split/phase points, metric pins, schedules and
  interruption reasons. Test cannot select the checkpoint.
- Robot DoF, controlled joint DoF and Action dimension are distinct. Declare
  ordered channel names, meaning, joint identity, units and representation.
  Per-channel MSE uses squared units; MAE/RMSE use original units. Record
  normalization and fit split. Required action reports cover every named channel
  with held-out MSE; add MAE/RMSE as available. Adapt the contract with evidence
  when this analysis is inapplicable. Never pool mixed units into one MSE.
- Evaluation pins exact inputs/protocol and preserves case/seed validity,
  success/failure, infrastructure counts and measured predicates.

Missing/unmeasured is not zero. A justified exclusion is a coverage decision,
not a missing measurement. Missing optional reports stay visible. Required report
failures, changed bytes, wrong inputs/checkpoints and inconsistent populations
prevent publication. Computation is not rerun solely for a publication error;
repair under the frozen contract and use the supported reindex/recovery path.

Training reports attach to Policies. Comparisons are derived views over accepted
evaluations; neither is a standalone Experiment. Observer weights and platform
smoke belong to their Tool/Component or operational evidence.

## Inspect

Result reads expose bounded `reporting` descriptors and exact input/adaptation
references. `results.read_report` selects an exact output revision, requirement and
report-relative JSON pointer. Arrays use `offset`/`limit`; follow `next_offset`.
The server verifies bytes and bounds each response to 256 KiB. One page is not the
complete measured population. Dashboard uses the same typed descriptors.

Legacy records remain readable. Do not invent old prompts, losses, spatial
coverage, DoF or lineage to make a template appear complete.

Registry Archive/Restore and deprecation use `components.inspect_lifecycle` followed
by `components.set_lifecycle`. See the Components guide for exact target selectors,
authorization and explicit batch selection. These overlays preserve sealed pins and
files; they are not execution or scientific acceptance gates. Discovery accepts
`include_archived` / `lifecycle_state`; exact reads remain available.
