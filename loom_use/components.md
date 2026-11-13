# Components, Tools, Profiles and dependencies

The Dashboard Runtimes workspace contains Profiles, Components, Tools and Runtimes views. Open a Profile to inspect its exact combination and follow links from a
Runtime back to consuming Profiles. The same combination is returned by
`components.profiles`; read-only clients can discover it without execution rights.

Profiles may declare optional usage metadata, for example:

```json
{"usage": {
  "label": "Generate and evaluate",
  "description": "One requested outcome with generation and evaluation stages",
  "categories": ["sdg", "evaluation"],
  "execution_modes": ["pilot", "full"]
}}
```

Use actual supported purposes, not names guessed from identifiers. Categories and
modes describe intent without imposing new Job gates. Existing Profile revisions
are not rewritten: missing purpose remains unrecorded and observed Job use is
separate. A pinned Tool is available to the runner; inspect source when actual
function invocation order matters. Stage environments may use different Runtime
revisions. Legacy/direct images have no inferred dependency inventory.

All software is shared. Built-in identifies Loom-supplied software, not a sharing
mode or ownership claim. Research-specific Ironfish/planner/policy code normally
keeps user attribution. To change current attribution, discover
`components.assign_owner` and provide the original `namespace`, `id`, extension
`kind` and target registered user ID (or `built-in` for a verified Loom-provided
item). This preserves namespace paths, revision pins and original provenance.

A Component is an immutable Job lifecycle adapter: request handling, execution entry and result return. Research algorithms (planners, policy/hybrid runners, transforms) belong to independently versioned Tools, which may be called repeatedly within one Job. A profile is an execution contract binding that revision to an image, commands, inputs, outputs, defaults and limits. Use components.write to edit, components.register to register, and components.register_profile for standalone profiles. Read each action's exact fields through system.describe. New components do not require new MCP servers or tools.

The runner receives a request JSON path as its final argument. The request includes params, output_dir, profile and Job context. Optional prepare/finalize/on_error/on_complete/on_cancel commands use the same runner format. External callbacks must deduplicate effects by event_id. Callback failures are reported separately from execution outcomes.

## Outputs

Write files and result.json into output_dir. The minimum result contains entities/metrics; entities include id, kind, path, provenance and relations. Record request.profile.revision and request.profile.sha256 as provenance.profile and provenance.producer_sha256 respectively. Declare domain semantics such as measurements, units, denominators and data splits in the profile's outputs/metadata_schemas. Generic Job state transitions do not generate domain metadata automatically.

## Installation and reuse

- `<configured-dependencies>/<repository>`: editable external source checkouts.
- `<configured-dependencies>/installations/<runtime>/<installation-attempt>`: Dockerfiles, patches, constraints/locks, build and validation records.
- Python environments and installed libraries: inside pinned execution images.
- `/loom/components`, `/loom/profiles`: registered user source and execution contracts.

Docker layer/build caches reduce installation work but do not preserve corrected source. Record local changes as recipes/patches, build a new image and select it in a new profile revision. Never overwrite existing image tags or active revisions. Source and recipes must remain sufficient to rebuild after cache deletion.

Standalone image profiles currently require an administrator-approved image already available in local Docker. Registration pins its digest. Jobs cannot request arbitrary host mounts, Docker sockets or privileged execution. The host executor owns network and image provisioning policy. Do not assume prepare has unrestricted Internet access.

External dependency checkouts and image recipes use the dependency directory selected in the installation plan. Read the runtimes guide for explicitly authorized administrator installation, exact Runtime revisions, dependency validation and Profile bindings. Connecting a client never installs a research framework implicitly. Keep datasets, assets and weights in configured data storage.

## Writable data stores

`system.status.data_stores` lists configured stores and their container paths. New installations default host `/loom/data` to `data/0`; additional directories are RW stores such as `data/1`. A profile runner can declare `"storage": ["data/1"]` on runtime, prepare or a lifecycle hook. The executor mounts only those registered names at `/loom/data/1`, and the runner receives `request.storage["data/1"]`. No host path is accepted in this declaration. Unknown stores fail registration/execution.

For example: `"runtime": {"executor":"container","runtime_id":"python","command":["python3","/component/run.py"],"storage":["data/1"]}`. Standalone profiles expose the same field through HTTP and MCP. Omit storage for only `/work` plus declared read-only artifact inputs. Explicit RW access can modify shared files; coordinate changes to existing datasets. Publish result.json and registered outputs through `request.output_dir` on the Job’s selected `output_storage`; ordinary shared files in additional stores remain visible in Explorer. Storage mounts do not relocate historical artifacts or change their hashes/locators.

## Owner

Registration defaults to the authenticated user. Use `owner: "built-in"` explicitly
for common components; their display label is **Built-in**. Shared visibility alone
does not make a user component Built-in. `components.list` returns every namespace
unless filtered with owner. Results include owner_profile (id/login/display_name),
owner_name and owner_kind; profile discovery carries the same responsibility.
The existing owner input/output remains the stable namespace used by get/write and
profile references. Existing responsibility records take precedence for display;
paths, source pins and submitted Jobs are unchanged. Unknown historical identities
remain visible by their recorded ID, not guessed or relabeled Built-in.

## Research Tools

Use components.list_tools/get_tool/register_tool within the existing six groups.
Tool registration accepts manifest/files, or an exact legacy component reference
(owner/id/revision). Copying a legacy snapshot preserves every old revision/Job.
Tool manifests carry research interfaces/provenance, not lifecycle profiles.
Profiles (or a Component manifest) declare tools as alias to exact owner/id/revision;
Loom seals tool_pins into accepted Jobs, verifies source hashes at launch, and
passes request.tools[alias] as /tools/<alias>, mounted read-only. New Tool revisions
do not change existing Jobs. Dependencies belong in the selected pinned image.
Tools share the existing administrator-granted source store under .tools.
Use the running server's system.describe for these optional actions. Older clients
can forward them without installing another MCP or changing authentication.

Dashboard groups Profiles, Components, Tools and Runtimes under the **Runtimes** workspace and
shows their exact combinations. Components implement Job lifecycle integration; Tools
implement reusable research algorithms, including hybrid runners. Profiles bind
execution contracts and immutable revisions. The navigation does not rename the
six MCP groups or the `components` actions.

Driver libraries and network policy are administrator-owned executor settings.
Defaults remain compute/utility and no network; explicit graphics or bridge egress
requires a supported host policy. Neither is an arbitrary Job/Profile Docker flag.
Image/source registration and an old receipt do not prove runtime acceptance.

Runtime is an independent execution environment, not a child revision generated by a Profile. A Profile selects Component/Tool and Runtime revisions. Optional manifest requires maps dependency names to exact versions; new bindings validate them against the selected Runtime inventory. See the runtimes guide.

## Reuse before registration

Before writing or registering software, call `components.profiles`, then inspect
candidate exact kinds with `components.get_profile`. Compare input/parameter and
output schemas, source/Tool pins, stage runtimes and limits with the requested
outcome. Submit another Job using a matching existing kind; put case IDs, seeds,
input locators and run settings in params/inputs. Do not clone a Component/Profile
merely to change a Job name, task ID, case or output directory. A retry/Attempt of
the same Job is not Profile reuse. Consolidated source Jobs are preserved history,
not independent current logical uses.

Register a new revision when source, interfaces, stage commands or environment
pins actually change. Preserve old revisions, report the concrete incompatibility
that required a new revision, and reuse unchanged Tools by exact revision. Discovery
is expected before creation; it is not an additional execution permission gate.

## Software categories

`components.annotate` attaches revision-scoped display tags and a concise label
with source evidence. Discover its schema and allowed tags through system.describe.
This does not rewrite sealed manifests, source files, namespaces, Job pins or access.
The Runtimes workspace has separate Profiles, Components, Tools and Runtimes views; Profile combinations
show Component and Tool columns separately. Categories describe software purpose;
a Tool alias describes its binding inside a particular Profile. Neither proves a
specific function was invoked. Profile usage statistics count distinct executed
logical Jobs, excluding consolidated sources and repeated Attempts of the same Job.

Profile runtime_bindings and combination stage dependencies are bounded previews.
Check dependency_count/dependencies_complete and use components.get_runtime with
the exact revision for the complete package inventory; do not infer absence from
an omitted preview entry. Stored execution pins remain unchanged.

Consolidated execution records remain associated with their canonical logical Job,
even when that Job uses another Profile kind. They increase recorded-execution
counts, not the number of independent Jobs/reuses. Profile details retain source
record links alongside the canonical Job link; consolidation must not make an
actually used Profile appear unused.

## Runtimes workspace curation and preparation

The single sidebar entry is Runtimes; its inner views are Profiles, Components,
Tools and Runtimes. Profile families retain exact revisions and can carry a
preferred/archive marker without changing execution selection. Use
components.curate for a concise display name, purpose/modes, grouping, declared
Tool roles or stage-specific representative dependency selectors. For a Runtime,
select an exact revision and primary_dependencies name/scope references. Curation
never installs packages, edits sealed definitions or migrates existing Jobs.

Read components.inspect_profile with kind and optional params to check defaults,
parameter schema, exact Runtime availability and declared dependency requirements.
It reports expected and observed versions, ambiguity and evidence scope; it does
not execute imports, reserve resources or establish GPU/scientific acceptance.
The returned draft is partial: fill required Job responsibility, notification,
category/mode and key fields using system.describe jobs.submit. Dashboard Prepare
Job checks parameters and copies an MCP request; no Job is enqueued automatically.
Compare exact definitions with components.compare_profiles before creating a new
revision. Changed paths exclude display metadata and never disclose env values.

stage_requires on a Component/Tool manifest is an array of stage/name/version
and optional scope requirements. Each declared stage must exist and its selected
Runtime must satisfy the exact scoped version at registration. Flat requires
keeps its main-stage behavior. Installed inventory is not a declaration of required
or invoked packages. Tool role metadata records intended placement; pinned
availability in a started Job is not function-call instrumentation.
