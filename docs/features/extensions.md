# extensions — Components, Tools, Profiles and execution environment contracts

Dashboard's software lists include historical records and
the latest native registration for each namespace/ID. Native revision summaries
retain source hashes and exact profile Tool references; usage distinguishes pins
to older Tools from current registrations. Independent profile references are
included. This is discovery, not validation that an image/dependency is healthy.
Older Component registrations that kept source pins only in sealed Profiles are
displayed using matching Profile evidence, without rewriting the legacy manifest.

## Responsibilities and boundaries

Registering a new component must not require new MCP tools or platform redeployment. Preserve profile revision pins on submitted Jobs.

## Inputs, outputs and authoritative sources

- [ops/runtime_contract.py](../../ops/runtime_contract.py)
- [ops/profiles.py](../../ops/profiles.py)

The linked code/schemas define fields, enums and state transitions. This document does not duplicate them.

## Call contracts

| Boundary | Input | Result or failure | Consumers |
|---|---|---|---|
| `ops/profile_store.py#operate` | Actor + profile operation/schema + revision/config | Stored validated profile revision or reference; submitted Job keeps its pin | api, execution |
| `ops/component_store.py#operate` | Actor + component operation/metadata | Immutable component revision and namespace; no new MCP server required | api, execution |

## Related features and change impact

[identity](identity.md)

When Profiles, Components or Tools change, check input schemas, prepare/run/completion hooks, executor runtime policy, registration/read MCP actions and guides.

## Tests and failure boundaries

- [tests/ops/test_components_platform.py](../../tests/ops/test_components_platform.py)
- [tests/ops/test_independent_profiles.py](../../tests/ops/test_independent_profiles.py)

Test sources define the concrete scenarios. `deploy/test.sh` records individual results as JSON/JUnit.

## Live validation

The execution-environment owner verifies actual external image dependencies, caches and GPU compatibility.

Owner presentation is additive: namespace remains immutable, owner_profile and
owner_name resolve current responsibility from item_owners and user_profiles.
Explicit common namespace built-in displays Built-in; user-owned components remain
user-owned when shared. Component list without owner filters lists all namespaces.
Tests cover another caller's shared discovery, Built-in, existing responsibility
and unchanged revision pins. No ownership-based visibility restriction is added.

Component source paths must be canonical relative file paths, without file/directory collisions or the reserved root `component.json` manifest. Editable symlink destinations are checked before publishing. Immutable source snapshots are published atomically; an identical snapshot left by a rolled-back database transaction can be reused on retry, while conflicting content remains untouched.

## Research software boundary

`ops/tool_store.py` registers immutable research Tools separately from Job lifecycle
Components. Exact pins are sealed into Profiles/Jobs, verified by runtime_client,
and mounted read-only by the executor. The six MCP groups are unchanged. Legacy
Component snapshots are copied only by explicit exact-revision selection; old
registrations and provenance remain intact. Dashboard groups Profiles, Components, Tools and Runtimes in one Runtimes workspace, retaining exact references. The `components` MCP group is unchanged.

Extension registration/source writes obey maintenance and readonly admission
fences in the same SQLite transaction as their state change. Discovery and exact
revision reads stay available. The fence cannot be bypassed by selecting the
components Tool actions or standalone Profile actions.

Execution kind identities are unique across Component-backed Profiles, independent
Profiles and installed configuration. Registration rejects dotted-name collisions
before publishing a snapshot; new software cannot shadow an accepted Job pin.

## Managed Runtimes

`ops/runtime_catalog.py` binds exact administrator Runtime revisions into Profile
stages and projects Profile/Job consumers. `executor/runtime_store.py` owns a
separate host `runtimes.sqlite3`; Ops never writes it or gains Docker/build access.
The executor validates instance grants and content hashes at each new launch.
Legacy runtime_id/image contracts retain their behavior and are labeled explicitly.
Component/Tool requires declarations check exact dependency inventory versions;
they do not resolve packages or certify research semantics.

`deploy/runtime.py` stages reviewed recipes and verified dependency downloads in
the configured installation dependency directory, builds an immutable image and
requires measured versions/checks before registration. A non-root/read-only smoke
is bounded and must return its container. An exclusive installation lock excludes
GPU dispatch and unconfirmed validation return persists a fence until reconciliation.
Builds/smokes are administrator actions, never implicit MCP entry or Job prepare.

The Runtimes workspace includes a Runtimes view with representative dependencies and exact validation scope,
and Profile/Job relationships. The six MCP groups remain unchanged; components
runtimes/get_runtime and the authenticated HTTP projection share one read contract.
Tests: tests/ops/test_runtime_catalog.py, tests/ops/test_executor_boundary.py and
tests/deploy/test_runtime_install.py. Real Isaac/GPU acceptance remains separate.

## Execution Profile combinations

The Runtimes workspace opens on Profiles, also linked from each Runtime's
consumers. It includes component-backed, standalone and installation-configured
exact Profile kinds, plus the built-in CPU check. A combination records Component
revision, exact Tool pins, stage-specific Runtime pins/dependencies, input/output
contracts and actual Job usage. Mounted Tools are available to container stages;
the inventory does not infer which function the runner invokes or its call order.
Legacy/direct-image package inventories remain unrecorded rather than empty.

Profile discovery bounds each repeated environment dependency preview to 16
name/version/kind/scope entries, with dependency_count and dependencies_complete.
Both runtime_bindings and combination stages use this projection; immutable
stored pins remain full. Full inventories are read from components.get_runtime
using the exact revision, also linked in Dashboard. This keeps large package
inventories from multiplying across Profiles beyond the installed client's 2 MiB
transport bound. Regression coverage models 60 Profiles with 996 dependencies.

Optional Profile `usage` declares a label/description, multiple Job categories and
multiple execution modes. These values reuse the Job classification vocabulary.
They do not add admission gates or change submitted Job metadata. Old revisions
remain unchanged: declared purpose, observed Job classification and matching
frozen Profile hashes are displayed separately. Profile discovery is readable
with read scope; read scope still cannot register or attribute software.

`components.assign_owner` changes current attribution for a Component, Tool or
standalone Profile using its original namespace/ID. `built-in` denotes Loom-supplied
software, never shared access: all workspace software remains shared. Namespace,
source bytes, revisions, frozen Jobs and original provenance remain unchanged.
The operation uses the normal submit scope, maintenance fence and ownership audit;
Built-in is an attribution marker, not a newly registered user or a signature.
The UI uses resolved responsibility rather than a legacy namespace as its owner
label and preserves the original namespace separately.

Profiles are the default view of the Runtimes sidebar entry, alongside Components, Tools and Runtimes. The default detail shows shared Component/Tool pins above stage-specific environments and dependency versions. Conditional hooks are labeled by their trigger. Job history, input/output settings and identity are collapsed; no Profile self-node or duplicate dependency table is shown. The built-in CPU check is projected from code even in an otherwise empty development catalog, with its real worker stage and no fabricated Component or registered Runtime. Development acceptance checks the running instance source mounts as well as fixture-backed browser tests.

Revision-scoped software annotations supply evidence-backed categories and concise
labels without changing sealed source/Job contracts. The Profile table separates
Components and Tools, and details keep aliases, categories and stage environments
visible without boilerplate definitions. Missing declared purpose can display
observed Job categories with an explicit tooltip/style distinction. Reuse counts
are distinct executed logical Jobs; consolidated sources and same-Job Attempts do
not increase reuse.

Administrator Runtime inventory probes measure immutable images and declared
mounted package/source metadata without a build, GPU launch or research execution.
Current observations bind the exact administrator policy, image ID and metadata
hashes; legacy runtime_id and direct-image execution fields stay unchanged. A
changed tag/policy/metadata cannot be silently presented as the current observation.
New Runtime bindings verify observed metadata again. Inventory-only validation
never certifies framework imports, graphics or scientific compatibility. Package
names with conflicting context versions cannot satisfy an ambiguous requirement.

Consolidated execution records remain associated with their canonical logical Job,
even when that Job uses another Profile kind. They increase recorded-execution
counts, not the number of independent Jobs/reuses. Profile details retain source
record links alongside the canonical Job link; consolidation must not make an
actually used Profile appear unused.

## Runtimes workspace and reusable execution workflows

The single sidebar entry is Runtimes. Its inner views are Profiles, Components,
Tools and Runtimes. Profile families derive from verified Component/profile-key or
standalone identities, retain exact revision links, and keep research and platform
checks separate. Preferred/archive metadata is display-only; it does not update
existing Jobs, change grants or select a different execution pin.

ops/runtime_hub.py stores display curation separately from immutable definitions.
Read-only inspect_profile validates supplied parameters and reports per-stage,
scoped dependency requirements; compare_profiles returns changed paths without env
values. Prepare Job creates a checked MCP draft, never an automatic submission.
The authenticated Dashboard action relay permits only inspect_profile,
compare_profiles and curate, retaining same-origin/CSRF and server scope checks.
Tool roles describe declared stage bindings, while source remains responsible for
invocation. Software pinned availability and exact Runtime executions are distinct
from current configuration associations and unmeasured Tool invocations.

Representative dependencies use owner-selected name/scope references to existing
inventory records. The full inventory remains immutable diagnostic evidence,
including deprecated entries, with no bulk package count on primary screens.
Managed-directory provenance comes from canonical paths on the host using the
configured root; image/host/external origins are separate and old unrecorded
origins remain unclassified. No directory cleanup, package installation, GPU
execution or operational migration follows from display curation or inspection.
