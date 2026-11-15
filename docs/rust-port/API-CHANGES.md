# Shared API and instruction contract plan

All names in the proposal table are planned capabilities, not available actions.
Freeze the reference six MCP groups and HTTP behavior first. HTTP, MCP, Dashboard,
CLI and retained clients call the same authorized application commands/queries.
A typed registry supplies argument schemas, required scopes, safe errors, compact
reference and guide links; do not hand-maintain duplicate dispatch/schema lists.

## Versioning and proposed additions

| Capability | Proposed surface | Validation / compatibility |
|---|---|---|
| Scenario list/detail/graph | `results.scenarios`, `results.get_scenario`, `results.scenario_graph` | Read scope; bounded cursors; unassigned and unavailable links visible |
| Scenario registration/revision/archive | `results.register_scenario`, `results.update_scenario`, `results.archive_scenario` | Submit/delegated write scope, stable key, expected revision for updates/archive |
| Explicit research links | `results.link_scenario`, `results.supersede_scenario_link` | Stable key and revision; validate target identity and edge semantics; no entity promotion |
| Job Scenario pin | Optional `scenario_ref` on submit/workflow | Resolve exact revision at acceptance; absent allowed for existing/operational runs |
| Rust draft templates | `components.scaffold` / template discovery | Pure archive/manifest generation; no host writes, build, registration or Job submit |
| Package validation | `components.validate_package` | Read-only report on a supplied draft; resource bounds, no executing build scripts |
| Source/build registration | Existing register/write plus versioned implementation/build refs | Preserve old request shapes; binary content uses verified locators |
| Historical current location | Additive `results.bind_historical_location` | Verify immutable historical identity/store/hash; recorded vs current availability separate |
| Queue/history purpose | Existing queries with explicit independent selectors | All real occupants for queue; history default All; purpose never schedule eligibility |

Naming/group ownership is fixed during D1 registry fixtures before client examples
are released; prefer these additive actions within the current groups to a seventh
hard-coded group. If a public break becomes necessary, create a declared revision
with an upgrade adapter rather than silently changing the meaning of an old field.

## Wire discipline

Requests decode into operation-specific DTOs. Preserve valid old defaults, empty vs
missing, null/zero distinctions, large integers, stable ordering and typed errors.
Reject new unsupported fields clearly. Historical JSON/unknown sealed fields have
a lossless codec rather than the new-input DTO's destructive normalization.
The manifest reports supported API/guide/client/SDK versions separately.

MCP initialize, list/call, JSON/SSE envelopes, notifications, protocol headers,
request IDs, scope-filtered schemas and structured errors retain reference
behavior. Error messages expose no credential, arbitrary upstream body, host path
or raw traceback; opaque incident locators are validated. Mutation response loss
returns uncertainty tied to the same key and never causes client auto-replay of
an unkeyed external effect.

## Guidance and examples

Guides are English, versioned and packaged from the selected source. Replace
stale static action enumerations with generated registry references. Example Cargo
projects, result manifests, workflow requests and Scenario links are validated
against actual schemas in CI. A guide references an action only when the registry
advertises it; older clients use capability discovery, not version guessing.

The server components/jobs/runtimes guides explain Rust-first authoring, exact
reuse/pins, justified Python integration and separate operational purpose. A Rust
API does not imply every submitted research dependency is Rust. Python integration
is allowed by concrete boundary, not a blanket escape for copied control-plane code.

## Consumer rollout

Run the reference pinned Loom Use first, then candidate source and its packaged
Skill release; exercise real HTTP/MCP calls in both. An independent Rust coordination client
receives additive snapshots through its existing transport and must retain one
logical Job link/report plus recovery semantics. No coordination-client rewrite, forced thread
recreation or Task/ACK gate is introduced. New fields must not turn running recovery
into failure or duplicate notifications. Browser consumers retain current layouts
until the Scenario/composition changes in their design are implemented.
