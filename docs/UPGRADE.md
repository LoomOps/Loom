# Administrator upgrade handoff

Use the exact source, images and checksums of the selected immutable release.
The external administrator (including an administrator Codex) owns execution;
release discovery never executes downloaded instructions or scripts.

## Inspect and plan

Run `deploy/update.py plan --instance INSTANCE --project PROJECT --release RELEASE`.
The result includes a cumulative dependency-ordered upgrade plan. All supported
steps ship in the target release under `deploy/migrations/`; intermediate release
downloads are unnecessary. Origins older than the declared minimum require a
bridge release. A missing dependency, cycle, changed completed script or checksum
mismatch is an error. Do not infer a migration from prose or concatenate obsolete
release-note commands. UI-only changes are already in the final image.

Each step has an immutable ID, introduction version, dependencies, bounded timeout,
and a release-manifest-bound Python script implementing check/apply/verify JSON
statuses. Completed receipts live privately in INSTANCE/runtime/migrations.json.
Checks inspect reality before recording completion, including after an interrupted
write. A migration must be idempotent and preserve old identities and revisions.
Keep old scripts unchanged in every future cumulative release.

## Apply or rebuild

`deploy/update.py apply` executes the plan after normal idle/executor fencing and
stopping writers. `--mode rebuild` additionally force-recreates the instance's
containers from the release images. This is a **state-preserving reinstallation**,
not the clean-host installer or uninstaller: the same instance paths, mounts,
installation/machine IDs, secrets and registries remain authoritative. It does not
reset Tailscale, revoke shares, issue tokens, or generate accounts. Choose this mode
explicitly in the plan; failure never triggers repeated automatic reinstallation.

A private snapshot covers configuration, credentials, admission/session state,
all instance SQLite databases, and the configured Component/Profile stores (Tool
snapshots share the Component store's `.tools` subtree). SQLite snapshots use its
backup API. File modes and service ownership are retained. External datasets,
weights, storage identities and host SSH/Tailscale state stay in place; no large
data relocation is inferred. Never publish snapshots or copy another host's state.

Before releasing admission, verify all prior database rows through their original
columns and all preserved configuration/extension file hashes, owners and modes.
New additive tables/rows are allowed. Account IDs, token digests/owners/scopes and
revocation state, original registry IDs/revisions/locators, and historical Job pins
must survive. Breaking transformations need an explicit compatibility-preserving
migration and acceptance checks, not a fresh database. Image health alone cannot
complete the update. Test existing authorized credentials and selected registry
reads through the actual deployed entry when performing live acceptance.

## Effective storage acceptance

`deploy/storage_parity.py --instance INSTANCE --project PROJECT` is read-only and
works without a saved original installation plan. It compares actual Docker bulk
and Explorer mounts, API/worker definitions, persistent marker IDs, the matching
host executor grant, SSH metadata, boot mount dependencies, and host/container
access as the service identity. The updater runs it before releasing admission;
installation doctor also reports this check. An HTTP 200 cannot bypass a mismatch.
No host path, identity, ACL or data is silently remapped. Drain and correct an
inconsistent installation explicitly while preserving prior result locators.

## Resume and rollback

Keep the update record path. A failed applying phase retains maintenance fencing.
Use `deploy/update.py resume --record RECORD` to recheck the release and receipts
and finish its owned operation. Do not rerun a fresh installation. Partial or
modified immutable source copies require rollback/review. Never force admission
open to suppress a preservation error.

`deploy/update.py rollback --record RECORD` refuses post-update database or
configuration writes, restores the saved databases/configuration and code, and
retains evidence. New work requires data-preserving recovery review. No migration
completion depends on a research agent ACK, coordinator task, or chat lifetime.

## Component / Tool transition

Components own the Job request, lifecycle entry and result contract. Tools are
research software called repeatedly inside a Job: planners, policy runners and
hybrid runners. Profiles bind exact software and environment revisions.

The additive `research-tools-v1` step creates the independent registry without
renaming any existing Component. Inspect sources and explicitly select candidates.
Use `components.register_tool` with `component: {owner, id, revision}` and a Tool
manifest to copy a verified exact legacy snapshot. The original Component, Profile
and Job references remain readable and runnable. Alternatively register UTF-8 Tool
source files. A Tool manifest cannot contain Job lifecycle commands/profiles.

Register a new thin lifecycle Component/Profile referencing exact Tools through
`tools: {planner: {owner, id, revision}}`. The resulting Profile and accepted Job
freeze Tool digests/source pins. The request provides `/tools/<alias>` paths, mounted
read-only by the restricted executor from the existing approved source store.
Image dependencies still require administrator-approved pinned images. Validate
the new Profile in an isolated Job before selecting it for new research. Do not
guess which S1v1/planner source should be reclassified or rewrite old Job specs.

Loom Use transports the same six MCP groups and discovers new action schemas.
An optional coordinator remains a user of Loom; it does not deploy this upgrade, mutate
operational registries, or activate native threads merely by reading this handoff.

## Release delivery

release.py emits `upgrade.json` alongside manifest.json. Publish that descriptor
with the source/guides/assets and include it in the release SHA256SUMS. It carries
the target, supported origin, modes and hashes of the cumulative scripts/guide.
Release discovery advertises repository-bound descriptor/instruction URLs only
when the descriptor asset is published. Downloading/reading it grants no execution
authority; verify against the immutable release before planning or running code.

Legacy source copying uses manifest pins or matching sealed Profile pins. Missing
or conflicting legacy evidence is rejected; the original record is never rewritten.
Preservation checks include database file ownership/modes. Rollback restores those
attributes and symlink ownership along with the original data and link targets.

Administrator executor configuration is fingerprinted independently of released
helper code. Custom runtime images, driver capabilities, network choices, asset
mounts and cache settings must retain their bytes/owner/mode across updates.
Unexpected edits require review before completing, resuming or rolling back.
Use the opt-in [runtime doctor](RUNTIME-DOCTOR.md) after upgrading a runtime boundary,
then perform the existing user's scientific smoke/acceptance separately.
