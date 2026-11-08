# installation — Compose, host layout, services and immutable builds/releases

## Responsibilities and boundaries

The loom service account, installation UUID and user profiles are independent. Separate dev and alpha databases, ports and data; restart only after execution resources return.

Start new installations with the [root INSTALL.md](../../INSTALL.md). Source builds and offline image bundles use the same installation plan.

## Inputs, outputs and authoritative sources

- [deploy/installer/install-plan.example.json](../../deploy/installer/install-plan.example.json)
- [ops/compose.loom.yaml](../../ops/compose.loom.yaml)

The linked code/schemas define fields, enums and state transitions. This document does not duplicate them.

## Call contracts

| Boundary | Input | Result or failure | Consumers |
|---|---|---|---|
| `deploy/install.py#install` | New instance/data paths, ports/images and service identity | Owned layout, independent installation UUID and initial config; refuses existing instance overwrite | network, execution, dashboard |
| `deploy/installer/setup.py#apply` | Verified immutable kit + complete installation plan | Installed services or explicit enrollment wait/failure; no false success from partial progress | network |

| `deploy/update.py#apply` | Verified release + idle installed instance | Image/helper update with saved rollback state; preserves host overrides and extensions | network, execution, dashboard, onboarding |

## Related features and change impact

[network](network.md) · [execution](execution.md) · [observations](observations.md) · [onboarding](onboarding.md) · [dashboard](dashboard.md)

When adding files or dependencies, check Dockerfiles, snapshots, installer HOST_FILES, Compose mounts and new-installation/upgrade guides.

## Tests and failure boundaries

- [tests/deploy/test_packaging.py](../../tests/deploy/test_packaging.py)
- [tests/deploy/test_installation.py](../../tests/deploy/test_installation.py)
- [tests/deploy/test_host_identity.py](../../tests/deploy/test_host_identity.py)
- [tests/deploy/test_install_kit.py](../../tests/deploy/test_install_kit.py)
- [tests/deploy/test_release_lifecycle.py](../../tests/deploy/test_release_lifecycle.py)

Test sources define the concrete scenarios. `deploy/test.sh` records individual results as JSON/JUnit.

## Live validation

Temporary-filesystem tests do not replace real clean-host installation, reboot and rollback verification.

## Release lifecycle

Source installation, final user overrides and idle update/rollback are defined in [UPDATES.md](../UPDATES.md). `test_release_lifecycle.py` covers real filesystem/SQLite preservation, busy rejection, failed startup recovery and refusal to discard new records. Docker/systemd are mocked there; fresh-host boot and actual release rollback remain explicit live checks.

## Release discovery boundary

`ops/release_updates.py` owns cached GitHub Release discovery independently of the Job
queue and SQLite. `deploy/release_check_setup.py` owns optional repository/channel/
interval/read-token setup. API and six-group MCP expose the same read/check contract;
Dashboard presents its cache. Sources, auth errors, rate limits, restart cache, channels,
ETags, pagination and nonblocking/coalesced checks are covered in
`tests/ops/test_release_updates.py`, `tests/ops/test_api_contract.py`,
`tests/deploy/test_release_check_setup.py` and Dashboard HTTP/browser tests.
No component installation, automatic updater or queue supervision is implied.

## Complete-service installation contract

The short root INSTALL.md routes to seven step documents under deploy/installer/steps.
After each step the agent rereads the index, runs the corresponding doctor stage,
and reports any unmet criterion. Recommendations come from host inspection, not
example values. Full native Tailscale identity, explicit Admin binding and a
separate Sharing Gateway are mandatory; a boolean tailscale plan is rejected.

`setup.py plan` generates linked configuration from this machine and the confirmed
human Admin. Primary output storage is separate from additional read-only catalog
stores. `compose-storage.yaml` is generated instance configuration; machine-owned
final overrides remain last and doctor inspects actual container mounts/images.

`doctor`/`verify` are read-only; `smoke` is the explicit bounded CPU Job entry point.
`handoff` reruns all checks and leads with the Gateway URL only after success.
External-client approval, real reboot and GPU computation are not implied by
local health checks. `resume-network` only republishes a matching installed plan
after configuration/services pass; it cannot reinstall the database/executor.
Other partial-stage failures must be diagnosed without deleting existing state.
Source packaging prepares the execution image explicitly and can resume only a
verified, completed immutable build. Offline kits include the same doctor,
smoke and all step documents.

Ingress acceptance also compares the live Docker bridge gateway, running Dashboard
trusted-proxy environment and Ops trusted proxies. A healthy HTTP listener or
`/session` reporting `sso: true` cannot prove identity forwarding. The Compose
network pins its gateway explicitly; tests cover IPAM selecting the dynamic pool
boundary instead of the expected first host address. Ask installers specifically
for the Admin account's **Tailscale login email**, not a generic administrator email.
