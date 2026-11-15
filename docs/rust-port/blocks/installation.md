# Installation, cumulative updates and matching artifacts — detailed design

Status: proposed Rust redesign; no implemented/deployed claim. Phases: D1, D7, D8.

## Responsibility and model

**Authority:** Retained administrator drivers + Rust migration/probe tools + immutable release kits.

**Typed model:** InstallPlan, ArtifactIdentity, StorageParityReport, MigrationGraph, MinimumWriter and RollbackReceipt.

**Operations:** Inspect/Plan/BuildKit/Apply/Doctor/AuthorizedSmoke/Update/Rollback; no automatic activation from import or client connect.

**Persistence:** Existing plan/config/overrides/installation UUID, services, DBs, source pins, credentials and independent Runtime/dependency paths.

## Reference source and consumers

- [INSTALL.md](../../../INSTALL.md)
- [deploy/build.py](../../../deploy/build.py)
- [deploy/build.sh](../../../deploy/build.sh)
- [deploy/compose.sh](../../../deploy/compose.sh)
- [deploy/executor-attach.py](../../../deploy/executor-attach.py)
- [deploy/guide_bundle.py](../../../deploy/guide_bundle.py)
- [deploy/host_identity.py](../../../deploy/host_identity.py)
- [deploy/host_services.py](../../../deploy/host_services.py)
- [deploy/install-kit.py](../../../deploy/install-kit.py)
- [deploy/install.py](../../../deploy/install.py)
- [deploy/installer/INSTALL.md](../../../deploy/installer/INSTALL.md)
- [deploy/installer/doctor.py](../../../deploy/installer/doctor.py)
- [deploy/installer/install-plan.example.json](../../../deploy/installer/install-plan.example.json)
- [deploy/installer/setup.py](../../../deploy/installer/setup.py)
- [deploy/installer/smoke.py](../../../deploy/installer/smoke.py)
- [deploy/installer/tailnet-plan.example.json](../../../deploy/installer/tailnet-plan.example.json)
- [deploy/loom-authority.service](../../../deploy/loom-authority.service)
- [deploy/migrations.py](../../../deploy/migrations.py)
- [deploy/migrations/catalog.json](../../../deploy/migrations/catalog.json)
- [deploy/migrations/research_tools.py](../../../deploy/migrations/research_tools.py)
- [deploy/policy_cleanup.py](../../../deploy/policy_cleanup.py)
- [deploy/release.py](../../../deploy/release.py)
- [deploy/release_check_setup.py](../../../deploy/release_check_setup.py)
- [deploy/runtime.sh](../../../deploy/runtime.sh)
- [deploy/snapshot.py](../../../deploy/snapshot.py)
- [deploy/source_install.py](../../../deploy/source_install.py)
- [deploy/state_snapshot.py](../../../deploy/state_snapshot.py)
- [deploy/storage_parity.py](../../../deploy/storage_parity.py)
- [deploy/test.sh](../../../deploy/test.sh)
- [deploy/uninstall.py](../../../deploy/uninstall.py)
- [deploy/update.py](../../../deploy/update.py)
- [docs/UPDATES.md](../../../docs/UPDATES.md)
- [executor/loom-executor.service](../../../executor/loom-executor.service)
- [ops/.dockerignore](../../../ops/.dockerignore)
- [ops/Dockerfile.loom](../../../ops/Dockerfile.loom)
- [ops/compose.data.yaml](../../../ops/compose.data.yaml)
- [ops/compose.dev.yaml](../../../ops/compose.dev.yaml)
- [ops/compose.executor.yaml](../../../ops/compose.executor.yaml)
- [ops/compose.loom.fence.yaml](../../../ops/compose.loom.fence.yaml)
- [ops/compose.loom.yaml](../../../ops/compose.loom.yaml)
- [ops/compose.observations.yaml](../../../ops/compose.observations.yaml)
- [ops/host_layout.py](../../../ops/host_layout.py)
- [ops/release_updates.py](../../../ops/release_updates.py)
- [deploy/runtime_doctor.py](../../../deploy/runtime_doctor.py)
- [deploy/queue_control_cli.py](../../../deploy/queue_control_cli.py)
- [deploy/boot_readiness.py](../../../deploy/boot_readiness.py)
- [deploy/runtime.py](../../../deploy/runtime.py)
- [deploy/runtime_inventory.py](../../../deploy/runtime_inventory.py)

Existing feature dependencies: network, execution, observations, onboarding, dashboard, diagnostics.
The references establish old behavior, not target Rust module boundaries.

## Implementation work

1. Package target-qualified Rust binaries/SDK/built-in packages, lockfile/source provenance, matching helper protocols, generated guides and retained web assets.

2. Keep prebuilt operator artifacts: no compiler installation on a target simply to update Loom. Client/SDK/build Runtime compatibility is explicit.

3. Extend snapshot manifests, Dockerfiles, installer HOST_FILES, Compose/systemd and source fingerprints; cargo files alone do not switch the installed executable.

4. Doctor compares effective fully merged mounts, roots/markers, API/worker/Executor/Explorer/SSH and service-user access, including installations without an old plan.

5. Use cumulative idempotent migrations and admin instructions/scripts. Skipped versions resolve all required edges or fail visibly; preserved rebuild/reinstall retains tokens/accounts/registries.

6. Fresh-install smoke uses the release-built Rust package through normal prepare/run/publication. It remains bounded operations/test evidence, not a research dataset/experiment.

7. Move the cached release-metadata checker to a bounded Rust read service; preserve channels, ETags, pagination, coalescing and safe error/cache semantics. It never downloads or deploys releases automatically.

8. Only after explicit deployment request drain execution/result/callback/Runtime owners, back up, switch immutable matching artifacts, verify identity and resume.

## Transaction and effect boundary

Installer is not a second Job state writer. Locks/epoch and Runtime validation fences guard maintenance; local health 200 or a progress marker cannot assert full readiness.

## Retained code and compatibility

Existing host/provisioning/auth drivers and installation choices. Keep machine overrides private and last in effective configuration.

Production cutover is D8 only and separately authorized. Verify each installation independently, preserve prior immutable artifacts and never overwrite a numbered release.

## Acceptance cases

| Case | Trigger | Required result |
|---|---|---|
| MIG-01 | Several releases skipped or rebuild required | All prerequisites applied once; credentials, IDs, registries/pins preserved. |
| MIG-02 | Explorer and worker use different host store | Doctor reports mismatch; no false ready or active silent remap. |
| INST-01 | Fresh host without Cargo | Verified native artifacts and bounded Rust smoke work. |
| INST-02 | Helper/binary/source mismatch or unknown owner | Refuse switch/readiness; retain concrete recovery evidence. |

Reference regression modules (adapt moved internals to actual Rust boundaries):

- [tests/deploy/test_host_identity.py](../../../tests/deploy/test_host_identity.py)
- [tests/deploy/test_install_doctor.py](../../../tests/deploy/test_install_doctor.py)
- [tests/deploy/test_install_kit.py](../../../tests/deploy/test_install_kit.py)
- [tests/deploy/test_installation.py](../../../tests/deploy/test_installation.py)
- [tests/deploy/test_packaging.py](../../../tests/deploy/test_packaging.py)
- [tests/deploy/test_public_source.py](../../../tests/deploy/test_public_source.py)
- [tests/deploy/test_release_check_setup.py](../../../tests/deploy/test_release_check_setup.py)
- [tests/deploy/test_release_lifecycle.py](../../../tests/deploy/test_release_lifecycle.py)
- [tests/deploy/test_storage_parity.py](../../../tests/deploy/test_storage_parity.py)
- [tests/deploy/test_uninstall.py](../../../tests/deploy/test_uninstall.py)
- [tests/deploy/test_upgrade_migrations.py](../../../tests/deploy/test_upgrade_migrations.py)
- [tests/ops/test_release_updates.py](../../../tests/ops/test_release_updates.py)
- [tests/deploy/test_runtime_doctor.py](../../../tests/deploy/test_runtime_doctor.py)
- [tests/deploy/test_boot_readiness.py](../../../tests/deploy/test_boot_readiness.py)
- [tests/deploy/test_runtime_install.py](../../../tests/deploy/test_runtime_install.py)

## Exit review

Validate the cases above plus the shared [domain](../DOMAIN.md), [API](../API-CHANGES.md),
[compatibility](../COMPATIBILITY.md) and [migration](../MIGRATION.md) contracts.
Record exact source/artifact hashes and which external boundaries remain unverified.
A passing old Python suite or compiling crate alone does not close this block.
