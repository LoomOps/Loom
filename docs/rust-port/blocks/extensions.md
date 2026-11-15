# Components, Tools, Profiles, Runtimes and dependencies — detailed design

Status: implemented development candidate; operational deployment not performed.
Concrete ownership, test boundaries and external limits: [IMPLEMENTATION](../IMPLEMENTATION.md). Phases: D1, D2, D3, D5.

## Responsibility and model

**Authority:** loom-software validation + loom-app registration + host Runtime registrar + loom-query composition.

**Typed model:** SoftwareOrigin, Namespace, ComponentRevision, ToolRevision, ProfileRevision, RuntimeRevision, DependencyRef, BuildReceipt.

**Operations:** Register/Write/Get Component and Tool; RegisterProfile; RegisterRuntime under host authority; CurateDisplay; Inspect/CompareComposition.

**Persistence:** Existing source revision directories, components/tools/execution_profiles, Runtime DB revisions/grants/installations and presentation annotations.

## Reference source and consumers

- [ops/component_store.py](../../../ops/component_store.py)
- [ops/profile_store.py](../../../ops/profile_store.py)
- [ops/profiles.py](../../../ops/profiles.py)
- [ops/tool_store.py](../../../ops/tool_store.py)
- [ops/runtime_catalog.py](../../../ops/runtime_catalog.py)
- [executor/runtime_store.py](../../../executor/runtime_store.py)
- [ops/profile_inventory.py](../../../ops/profile_inventory.py)
- [ops/software_metadata.py](../../../ops/software_metadata.py)
- [ops/runtime_hub.py](../../../ops/runtime_hub.py)
- [ops/dependency_catalog.py](../../../ops/dependency_catalog.py)

Existing feature dependencies: identity.
The references establish old behavior, not target Rust module boundaries.

## Implementation work

1. Keep Component lifecycle responsibility, Tool reusable software, Profile exact combination and Runtime installed execution environment distinct. Profile is not a package installer.

2. Registration validates immutable source/build/pins without executing source/build scripts. Resolve stage-specific Runtime/Tool requirements under exact grants and lock at publication.

3. New pure orchestration and platform packages default to Rust; source/build receipts describe justified Python integration. Preserve old unrecorded language/image/runtime evidence without guessing.

4. Show Built-in origin only for release-supplied packages. Preserve shared usability, stable namespaces and exact provenance while changing display attribution independently.

5. Dependencies derive from configured-root Runtime receipts, with representative packages/categories in summary and exact inventory on demand. A list of installed packages is not proof every Job imported them.

6. Expose composition edges by stage, tool role/category, runtime/dependency pin and actual distinct logical-Job use. Do not inflate reuse with retries, nodes, smoke acceptance or imported duplicates.

7. Runtime builds/validation retain their independent durable fence and explicit host authority. No API-side package installation or silent conversion of direct-image legacy profiles.

## Transaction and effect boundary

Source/build candidates stage outside DB locks; registration and exact pins commit under admission/epoch guards. Cross-host/Runtime DB changes use explicit registrar intents/receipts and one writer authority.

## Retained code and compatibility

User source bytes and scientific Runtime ecosystems; existing administrator Runtime recipe tooling initially. Rust replaces registry decisions, not PyTorch/Isaac implementations.

Preserve all sealed revisions and Runtime DB/grants. New metadata is versioned/additive; conversions register new explicit revisions. Built-in smoke uses source-qualified identities.

## Acceptance cases

| Case | Trigger | Required result |
|---|---|---|
| EXT-01 | New Rust package with forged build hash | Reject before registration or launch. |
| EXT-02 | Profile uses same Tool in two stages/Runtimes | Composition displays exact stage bindings and validates both. |
| EXT-03 | Legacy direct image lacks receipt | Show recorded evidence/unrecorded fields; do not auto-repin. |
| EXT-04 | Runtime installer validates on GPU during upgrade | Fence conflicts; no force stop or false idle. |

Reference regression modules (adapt moved internals to actual Rust boundaries):

- [tests/ops/test_components_platform.py](../../../tests/ops/test_components_platform.py)
- [tests/ops/test_independent_profiles.py](../../../tests/ops/test_independent_profiles.py)
- [tests/ops/test_research_tools.py](../../../tests/ops/test_research_tools.py)
- [tests/ops/test_runtime_catalog.py](../../../tests/ops/test_runtime_catalog.py)
- [tests/ops/test_profile_inventory.py](../../../tests/ops/test_profile_inventory.py)
- [tests/ops/test_runtime_hub.py](../../../tests/ops/test_runtime_hub.py)
- [tests/ops/test_dependency_catalog.py](../../../tests/ops/test_dependency_catalog.py)

## Exit review

Validate the cases above plus the shared [domain](../DOMAIN.md), [API](../API-CHANGES.md),
[compatibility](../COMPATIBILITY.md) and [migration](../MIGRATION.md) contracts.
Record exact source/artifact hashes and which external boundaries remain unverified.
A passing old Python suite or compiling crate alone does not close this block.
