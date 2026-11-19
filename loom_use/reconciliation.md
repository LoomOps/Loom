# Reconcile selected assets

For an import, a relocated copy or a derived Dataset, select the matching [catalog case](catalog-cases.md) first.
Discover results.reconcile_contract and current schemas before proposing changes.
reconcile_inspect pages registered metadata; it never scans unregistered directories.
Agents identify meaning and exact source provenance, then use reconcile_plan with
a stable key, reconcile_validate, reconcile_apply and reconcile_get.
A plan holds at most 200 explicit operations and 256 KiB of metadata. Validation hashes
only selected candidates off the writer. Apply rechecks stamps, configured grants
and immutable target identity. A changed source or contract requires revalidation/a
new plan. Repeated successful apply returns the original receipt.

Use adopt for an existing exact native record, bind_file to add a matching copy,
and bind_historical for a full source/kind/id/revision/snapshot/file selector.
For an external import, declare the source system, ID, revision, optional
snapshot and original evidence. Use `loom-asset/v3` with
`definition.kind/label/primary`, an exact metadata template and canonical
`definition.metadata.research`. Each Dataset, Policy or Experiment import requires `definition.scenario_ref:{id,revision}` for both catalog classes. Generic technical evidence can be imported as an Artifact without a Task. Search existing Tasks, Datasets and Policies before proposing new records. Supply
`definition.relations` with `{kind,id,revision}` entries under the same Task:
`trained_on` for a Policy's Dataset, and `evaluated_on` and `evaluated_model` for
an Experiment's Dataset and Policy. Existing historical records retain their
original identities and source metadata.
V1/v2 remain legacy adapters. Conflicting source reuse fails.
Candidate locators use this installation's granted machine/storage identities and
relative refs. Optional original sha256/size must match. Stage bytes separately;
the reconciler never guesses paths, moves whole trees, installs software or creates
Jobs/Attempts. Import validation proves structure and bytes, not scientific acceptance.
Historical recorded availability stays immutable beside current copy bindings.

Use the `items` guide for discovery and registered reads; administrator index maintenance is separate from asset reconciliation.
