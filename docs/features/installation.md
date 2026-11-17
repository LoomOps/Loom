# installation — Compose, host layout, services and immutable builds/releases

## Boundary

The loom service account, installation UUID and user profiles are independent. Separate dev and alpha databases, ports and data; restart only after execution resources return.
New installation has one service contract: native Tailscale identity, Admin binding, Sharing Gateway, Ops, Dashboard, executor and explicit storage. Token-only fallback is not installation success.
Doctor observes actual state independently of progress receipts. Only explicit smoke submits a Job; diagnostics never repair or grant access. Handoff leads with the verified Gateway URL.

Native installation smoke submits with `owner:system` and the real notification
recipient, so it is Dashboard-hidden from creation. A prior smoke receipt without
System attribution is repaired idempotently before it is reused. User Jobs do
not inherit System responsibility from a Profile, Runtime or smoke execution mode.

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Fresh layout | `deploy/install.py#install` | New instance/data paths, ports/images and service identity | Owned layout, independent installation UUID and initial config; refuses existing instance overwrite | network, execution, dashboard |
| Installation workflow | `deploy/installer/setup.py#apply` | Verified immutable kit + complete installation plan | Installed services or explicit enrollment wait/failure; no false success from partial progress | network |
| Idle update / rollback | `deploy/update.py#apply` | Verified built release + installed instance; idle Jobs and returned executor tickets | Updated images/helpers; overrides and extensions preserved; failure keeps recovery record | network, execution, dashboard, onboarding |
| Installation diagnostics | `deploy/installer/doctor.py#doctor` | Complete plan and installed instance; optional stage | Read-only per-check pass/fail, next step and explicit live-validation limits | network, identity, execution, observations, onboarding |
| Packaged acceptance Job | `deploy/installer/smoke.py#run` | Authorized installation and its matching kit | Bounded, repeatable CPU prepare/run/result test; never changes queue ordering | execution, results |

## Sources and contracts

- [Cargo.lock](../../Cargo.lock) — integration, packaging or documentation.
- [Cargo.toml](../../Cargo.toml) — integration, packaging or documentation.
- [INSTALL.md](../../INSTALL.md) — integration, packaging or documentation.
- [deploy/boot_readiness.py](../../deploy/boot_readiness.py) — integration, packaging or documentation.
- [deploy/build.py](../../deploy/build.py) — integration, packaging or documentation.
- [deploy/build.sh](../../deploy/build.sh) — integration, packaging or documentation.
- [deploy/compose.sh](../../deploy/compose.sh) — integration, packaging or documentation.
- [deploy/executor-attach.py](../../deploy/executor-attach.py) — integration, packaging or documentation.
- [deploy/guide_bundle.py](../../deploy/guide_bundle.py) — integration, packaging or documentation.
- [deploy/host_identity.py](../../deploy/host_identity.py) — integration, packaging or documentation.
- [deploy/host_services.py](../../deploy/host_services.py) — integration, packaging or documentation.
- [deploy/install-kit.py](../../deploy/install-kit.py) — integration, packaging or documentation.
- [deploy/install.py](../../deploy/install.py) — integration, packaging or documentation.
- [deploy/installer/INSTALL.md](../../deploy/installer/INSTALL.md) — integration, packaging or documentation.
- [deploy/installer/doctor.py](../../deploy/installer/doctor.py) — integration, packaging or documentation.
- [deploy/installer/install-plan.example.json](../../deploy/installer/install-plan.example.json) — integration, packaging or documentation.
- [deploy/installer/setup.py](../../deploy/installer/setup.py) — integration, packaging or documentation.
- [deploy/installer/smoke.py](../../deploy/installer/smoke.py) — integration, packaging or documentation.
- [deploy/installer/tailnet-plan.example.json](../../deploy/installer/tailnet-plan.example.json) — integration, packaging or documentation.
- [deploy/loom-authority.service](../../deploy/loom-authority.service) — integration, packaging or documentation.
- [deploy/migrations.py](../../deploy/migrations.py) — integration, packaging or documentation.
- [deploy/migrations/catalog.json](../../deploy/migrations/catalog.json) — integration, packaging or documentation.
- [deploy/migrations/research_tools.py](../../deploy/migrations/research_tools.py) — integration, packaging or documentation.
- [deploy/migrations/rust_authority.py](../../deploy/migrations/rust_authority.py) — integration, packaging or documentation.
- [deploy/native_artifacts.py](../../deploy/native_artifacts.py) — integration, packaging or documentation.
- [deploy/policy_cleanup.py](../../deploy/policy_cleanup.py) — integration, packaging or documentation.
- [deploy/queue_control_cli.py](../../deploy/queue_control_cli.py) — integration, packaging or documentation.
- [deploy/release.py](../../deploy/release.py) — integration, packaging or documentation.
- [deploy/release_check_setup.py](../../deploy/release_check_setup.py) — integration, packaging or documentation.
- [deploy/runtime.py](../../deploy/runtime.py) — integration, packaging or documentation.
- [deploy/runtime.sh](../../deploy/runtime.sh) — integration, packaging or documentation.
- [deploy/runtime_doctor.py](../../deploy/runtime_doctor.py) — integration, packaging or documentation.
- [deploy/runtime_inventory.py](../../deploy/runtime_inventory.py) — integration, packaging or documentation.
- [deploy/rust_build.py](../../deploy/rust_build.py) — integration, packaging or documentation.
- [deploy/snapshot.py](../../deploy/snapshot.py) — integration, packaging or documentation.
- [deploy/source_install.py](../../deploy/source_install.py) — integration, packaging or documentation.
- [deploy/state_snapshot.py](../../deploy/state_snapshot.py) — integration, packaging or documentation.
- [deploy/storage_parity.py](../../deploy/storage_parity.py) — integration, packaging or documentation.
- [deploy/test.sh](../../deploy/test.sh) — integration, packaging or documentation.
- [deploy/uninstall.py](../../deploy/uninstall.py) — integration, packaging or documentation.
- [deploy/update.py](../../deploy/update.py) — integration, packaging or documentation.
- [docs/UPDATES.md](../../docs/UPDATES.md) — integration, packaging or documentation.
- [executor/loom-executor.service](../../executor/loom-executor.service) — integration, packaging or documentation.
- [ops/.dockerignore](../../ops/.dockerignore) — integration, packaging or documentation.
- [ops/Dockerfile.loom](../../ops/Dockerfile.loom) — integration, packaging or documentation.
- [ops/Dockerfile.rust](../../ops/Dockerfile.rust) — integration, packaging or documentation.
- [ops/compose.data.yaml](../../ops/compose.data.yaml) — integration, packaging or documentation.
- [ops/compose.dev.yaml](../../ops/compose.dev.yaml) — integration, packaging or documentation.
- [ops/compose.executor.yaml](../../ops/compose.executor.yaml) — integration, packaging or documentation.
- [ops/compose.loom.fence.yaml](../../ops/compose.loom.fence.yaml) — integration, packaging or documentation.
- [ops/compose.loom.yaml](../../ops/compose.loom.yaml) — integration, packaging or documentation.
- [ops/compose.observations.yaml](../../ops/compose.observations.yaml) — integration, packaging or documentation.
- [ops/compose.rust.yaml](../../ops/compose.rust.yaml) — integration, packaging or documentation.
- [ops/host_layout.py](../../ops/host_layout.py) — active installer configuration/layout helper.
- [ops/release_updates.py](../../ops/release_updates.py) — retained release-check adapter; native service checking is rust/crates/ops/src/updates.rs.
- [rust-toolchain.toml](../../rust-toolchain.toml) — integration, packaging or documentation.
- [rust/crates/ops/src/updates.rs](../../rust/crates/ops/src/updates.rs) — native implementation / contract.
- [deploy/migrations/item_index.py](../../deploy/migrations/item_index.py) — integration, packaging or documentation.
- [deploy/migrations/reconciliation.py](../../deploy/migrations/reconciliation.py) — integration, packaging or documentation.
- [rust/contracts/reconcile-schema.sql](../../rust/contracts/reconcile-schema.sql) — native implementation / contract.
- [deploy/installer/steps/01-plan.md](../../deploy/installer/steps/01-plan.md) — integration, packaging or documentation.
- [deploy/installer/steps/02-host.md](../../deploy/installer/steps/02-host.md) — integration, packaging or documentation.
- [deploy/installer/steps/03-images.md](../../deploy/installer/steps/03-images.md) — integration, packaging or documentation.
- [deploy/installer/steps/04-configuration.md](../../deploy/installer/steps/04-configuration.md) — integration, packaging or documentation.
- [deploy/installer/steps/05-services.md](../../deploy/installer/steps/05-services.md) — integration, packaging or documentation.
- [deploy/installer/steps/06-network.md](../../deploy/installer/steps/06-network.md) — integration, packaging or documentation.
- [deploy/installer/steps/07-handoff.md](../../deploy/installer/steps/07-handoff.md) — integration, packaging or documentation.
- [docs/UNINSTALL.md](../../docs/UNINSTALL.md) — integration, packaging or documentation.
- [docs/UPGRADE.md](../../docs/UPGRADE.md) — integration, packaging or documentation.

## Change review

When adding files or dependencies, check Dockerfiles, snapshots, installer HOST_FILES, Compose mounts and new-installation/upgrade guides.

Related blocks: [network](network.md) · [execution](execution.md) · [observations](observations.md) · [onboarding](onboarding.md) · [dashboard](dashboard.md) · [diagnostics](diagnostics.md).

## Verification

- [tests/deploy/test_boot_readiness.py](../../tests/deploy/test_boot_readiness.py)
- [tests/deploy/test_host_identity.py](../../tests/deploy/test_host_identity.py)
- [tests/deploy/test_install_doctor.py](../../tests/deploy/test_install_doctor.py)
- [tests/deploy/test_install_kit.py](../../tests/deploy/test_install_kit.py)
- [tests/deploy/test_installation.py](../../tests/deploy/test_installation.py)
- [tests/deploy/test_native_artifacts.py](../../tests/deploy/test_native_artifacts.py)
- [tests/deploy/test_packaging.py](../../tests/deploy/test_packaging.py)
- [tests/deploy/test_public_source.py](../../tests/deploy/test_public_source.py)
- [tests/deploy/test_release_check_setup.py](../../tests/deploy/test_release_check_setup.py)
- [tests/deploy/test_release_lifecycle.py](../../tests/deploy/test_release_lifecycle.py)
- [tests/deploy/test_runtime_doctor.py](../../tests/deploy/test_runtime_doctor.py)
- [tests/deploy/test_runtime_install.py](../../tests/deploy/test_runtime_install.py)
- [tests/deploy/test_storage_parity.py](../../tests/deploy/test_storage_parity.py)
- [tests/deploy/test_uninstall.py](../../tests/deploy/test_uninstall.py)
- [tests/deploy/test_upgrade_migrations.py](../../tests/deploy/test_upgrade_migrations.py)
- [tests/ops/test_release_updates.py](../../tests/ops/test_release_updates.py)
- [tests/rust/test_installation.py](../../tests/rust/test_installation.py)
- [tests/rust/updates.rs](../../tests/rust/updates.rs)
- [tests/rust/application.rs](../../tests/rust/application.rs)
- [tests/rust/test_service.py](../../tests/rust/test_service.py)
- [tests/rust/test_item_image.py](../../tests/rust/test_item_image.py)

Acceptance outside these fixtures:
- Temporary-filesystem tests do not replace real clean-host installation, reboot and rollback verification.

Update admission rejects pending/running lifecycle callbacks and every unresolved
native execution effect, including when its Job is already terminal. The database
fence and idle checks share one transaction; container status alone does not prove
that a callback or pending native start has returned.

Image guide generation fails if the native binary cannot produce its bundle; it
does not substitute the retained Python reference. Offline kits include the
independent client acquisition helper and exact client descriptor used by handoff.
Update, deployment pause and uninstall resume reject pending/running callbacks
and unresolved native execution ownership before changing services.
