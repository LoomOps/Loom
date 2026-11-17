# Reconcile selected assets


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
Use import only when explicitly bringing an external asset into the current
contract. Declare source.system/id/revision and optional snapshot plus original
evidence; use schema=loom-asset/v3 and definition.kind/label/primary, typed
metadata, Research Task and relations. V1/v2 remain explicit legacy adapters;
the v1 adapter maps type/name/properties into a legacy v2 definition. Unknown schemas and conflicting source reuse fail.
Candidate locators use this installation's granted machine/storage identities and
relative refs. Optional original sha256/size must match. Stage bytes separately;
the reconciler never guesses paths, moves whole trees, installs software or creates
Jobs/Attempts. Import validation proves structure and bytes, not scientific acceptance.
Historical recorded availability stays immutable beside current copy bindings.


Use the `items` guide for discovery and registered reads; administrator index maintenance is separate from asset reconciliation.

For a new typed Dataset/Model/Experiment import, use `loom-asset/v3` and select an
exact registered template in `definition.metadata.template_ref`. Supply canonical
`definition.metadata.research`, including explicit robot recording status. Read
`metadata` before adapting a comparable existing record. V1/v2 adapters remain
legacy compatibility contracts and do not imply template validation.
