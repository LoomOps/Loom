# Live guides, manifests and client coordination — detailed design

Status: implemented development candidate; operational deployment not performed.
Concrete ownership, test boundaries and external limits: [IMPLEMENTATION](../IMPLEMENTATION.md). Phases: D1, D2, D6.

## Responsibility and model

**Authority:** loom-contracts generated registry + immutable server guides; independent Loom Use package.

**Typed model:** GuideRevision, ContractRevision, ClientReleasePin, SdkCompatibility and BuildIdentity.

**Operations:** system.status/guide/describe/contract and versioned guide bundle reads; client setup/update remain user-side operations.

**Persistence:** Immutable packaged guides/schemas/examples and declared client-release descriptor; named client credentials/profiles remain external.

## Reference source and consumers

- [dashboard/build_version.py](../../../dashboard/build_version.py)
- [ops/build_version.py](../../../ops/build_version.py)
- [ops/onboarding.py](../../../ops/onboarding.py)
- [loom_use/client-release.json](../../../loom_use/client-release.json)
- [deploy/client_release.py](../../../deploy/client_release.py)
- [tests/ops/client_source.py](../../../tests/ops/client_source.py)
- [loom_use/runtimes.md](../../../loom_use/runtimes.md)

Existing feature dependencies: identity, api.
The references establish old behavior, not target Rust module boundaries.

## Implementation work

1. Generate API references from the same Rust registry used by dispatch and scope filtering; correct stale PLATFORM/static enumerations.

2. Publish Scenario, Rust authoring/package/build and Runtime installation instructions as server-owned guides; guide-only updates cannot change accepted Job pins.

3. Version SDK/producer protocol separately from server/client. A guide advertises only implemented capabilities and includes older-server fallback instructions.

4. Co-release changed Loom Use Skills/examples with a real immutable client artifact, then update Loom pin and verify installed cache/refresh behavior.

5. Test main/support/operations classification, exact Profile reuse and Rust/hybrid authoring through behavioral fixtures. A string-match test for Rust is insufficient.

## Transaction and effect boundary

Read endpoints are side-effect free. Guide cache refresh/client installation does not grant server/host permissions, build software, issue tokens or restart a daemon.

## Retained code and compatibility

Loom Use Python stdio/connection/credential/refresh tooling. Server-owned schemas remain authoritative; do not copy them into an independent Rust client implementation.

Update bundle inventories/hashes, client release pin and cross-repo tests together. A source-only plan does not update any installed Skill.

## Acceptance cases

| Case | Trigger | Required result |
|---|---|---|
| ONB-01 | Guide changes but client pin stays | Refresh only changed verified guides; credentials and Jobs unchanged. |
| AUTH-01 | Agent fixture requests a new pure lifecycle runner | Rust template/reuse path; Python exception only for declared external integration. |
| ONB-02 | Old server lacks new authoring/Scenario API | Discover capability and use supported contract; no invented fields. |
| ONB-03 | Published package vs mutable source | Accurate source/guide/SDK identity; no false immutable report. |

Reference regression modules (adapt moved internals to actual Rust boundaries):

- [tests/ops/test_onboarding.py](../../../tests/ops/test_onboarding.py)
- [tests/ops/test_client_setup.py](../../../tests/ops/test_client_setup.py)
- [tests/deploy/test_client_release.py](../../../tests/deploy/test_client_release.py)

## Exit review

Validate the cases above plus the shared [domain](../DOMAIN.md), [API](../API-CHANGES.md),
[compatibility](../COMPATIBILITY.md) and [migration](../MIGRATION.md) contracts.
Record exact source/artifact hashes and which external boundaries remain unverified.
A passing old Python suite or compiling crate alone does not close this block.
