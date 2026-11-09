# extensions — Dynamic components, profiles and execution environment contracts

Dashboard's shared Components & Tools inventory includes historical records and
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

When profiles or components change, check input schemas, prepare/run/completion hooks, executor runtime policy, registration/read MCP actions and guides.

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
registrations and provenance remain intact. Dashboard lists Tools independently.
