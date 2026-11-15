# Installation — Item/index impact and detailed work

Status: proposed. Impact: **Direct**. No runtime change is made by this plan.

## Observed baseline

The native candidate delivers same-image helper binaries and schema 5 via an immutable cumulative cutover. Existing installation paths/settings and private Runtime roots are preserved.

Reviewed source:

- [deploy/install.py](../../../deploy/install.py)
- [deploy/source_install.py](../../../deploy/source_install.py)
- [deploy/release.py](../../../deploy/release.py)
- [deploy/snapshot.py](../../../deploy/snapshot.py)
- [deploy/migrations/catalog.json](../../../deploy/migrations/catalog.json)
- [deploy/migrations/rust_authority.py](../../../deploy/migrations/rust_authority.py)
- [deploy/installer/doctor.py](../../../deploy/installer/doctor.py)
- [ops/Dockerfile.rust](../../../ops/Dockerfile.rust)
- [ops/compose.rust.yaml](../../../ops/compose.rust.yaml)

## Implementation / consumer review

1. Add explicit local index/metadata/spool settings, ownership/mode, quotas, projection/collector lifecycle and source retention settings without hardcoded research paths.
2. Introduce a new immutable cumulative migration/backfill step; do not change the already numbered native cutover.
3. Deliver matching native binaries, guide/schema/extraction versions, manifests and license/FTS capabilities in source/offline kits.
4. Doctor checks core authority, descriptor preservation, host capability and search readiness separately. Index rebuilding is not complete search acceptance or an execution gate.
5. Preserve index/metadata roots on update/reinstall; define safe derived-index discard and refuse unsafe authority rollback after new writes.
6. Run clean-layout fixture and real CPU native image acceptance; full fresh-host/systemd/reboot/provider rollout remains separately requested.

## Invariants

- No automatic deployment, new root privilege, Docker-group membership or socket mount.
- Dependency directory and existing Runtime/User source state remain untouched.
- Failure reports identify the failing stage without a false ready flag.

## Planned acceptance and dependencies

Cases: `TX-04`, `TX-06`, `MIG-01`, `MIG-02`, `MIG-03`, `LOG-02`, `VERIFY-01`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [portability](portability.md), [execution](execution.md), [onboarding](onboarding.md).
