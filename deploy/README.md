# Loom deployment

Loom uses a host service account `loom:loom`. Developer checkouts remain owned by
individual developers. Docker Compose creates containers, while the host installer
creates the account, `/loom` layout, configuration and observation service first.
The account has no login password, interactive shell, sudo or Docker group access.

## Source installation (recommended)

Start with the repository [INSTALL.md](../INSTALL.md). It pins a published tag, builds
images and produces a local installation kit using `source_install.py`. No image
registry or tar export is required. Existing instances use [UPDATES.md](../docs/UPDATES.md).

## Offline installation bundle

`install-kit.py` packages an already built immutable release with its Ops,
Dashboard and Python runtime images, version-matched Compose/host installers,
an agent installation guide, a declarative installation plan and the user guide
bundle. It does not copy existing `/loom` state or credentials.

```bash
python3 deploy/install-kit.py \
  --release /loom/builds/alpha/<release-version> \
  --output /path/to/new/loom-install-kit \
  --guide /path/to/loom-agent-kit-<release-version>.zip
```

Give the resulting archive to the target user's agent with instructions to read
`INSTALL.md`. The agent inspects the host, discusses storage/dependency/GPU needs,
and applies the agreed plan using `setup.py`. The installer supports existing
filesystem UUID mounts without formatting, optional pinned Git sources under
`/dependencies`, and a systemd Compose unit that waits for storage mounts.
Host OS packages, GPU drivers and optional research environments are separate.
This is a new-machine installation path; it refuses existing Loom instances.

## Internal host helpers

`install.py`, `executor-attach.py`, `tailnet.py` and `access_setup.py` are implementation
helpers for the numbered installation workflow and scoped maintenance. Running a
subset does not constitute a supported installation. Use the full installer and
doctor; do not substitute a token-only Dashboard or omit Gateway/identity.

The host installer creates/reuses `loom:loom` and applies numeric UID/GID to Compose.
It refuses conflicting existing accounts and preserves existing data ownership.
Account creation is host-side, not a privileged Compose bootstrap container.
Credentials remain in protected local files and are never part of user handoff.

## Ownership and host services

- `/loom`, `/loom/shared`, executor code/config and deployed host-helper code are
  root-managed, with no service-user write access to their parents.
- Ops configuration/DB/state, profiles/components, Job outputs and new build
  snapshots belong to loom. Historical file content, IDs and credentials stay intact.
- `/loom/shared/observations` is a protected host output tree. Each instance mounts
  only its own filtered observation directory read-only. `loom-observations.timer`
  runs in system systemd, independent of the operator's login session.
- Optional handoff authority uses `User=loom` under system systemd and root-managed
  code in `/loom/shared/services`. Existing state/epoch/session and TLS keys are
  preserved. `host_services.py authority` requires existing authority state.
- Development source remains user-owned. Only `dashboard/public` needs a narrow
  named-user/default ACL for loom to rebuild live HTML; never chown the whole checkout.
- Shared external dependencies such as `/dependencies/example-library`, original research
  data and archives are not automatically taken over by the service account.

Host observations may be attached to another installed instance with:

```bash
sudo python3 deploy/host_services.py observations /loom/ops/dev-build \
  --project loom-dev-build --uid "$(id -u loom)" --gid "$(id -g loom)" --show-dev
```

Do not recursively chown `/loom` wholesale. Use the migration manifest/backup and
select mutable trees. Transition executor grants, socket access, Compose identity,
external output storage and system units together, after actual executions return.

## Development and releases

- `compose.sh`: isolated dev-build. `runtime.sh`: Example Machine alpha.
- `build.sh`: numbered dev snapshot/images, no automatic service restart.
- `release.py VERSION`: immutable alpha snapshot/images, no deployment.
- `test.sh --browser`: isolated regression tests.

Build tools escalate for protected `/loom/builds` paths when needed, then assign new
snapshots to loom. Existing numbered snapshots/images are never overwritten.
Images contain code; state/credentials/data are external. Existing dev live-source
updates do not restart active Jobs. Select a new `build.env` and recreate the intended
instance only after executions/callbacks and executor tickets have returned.

Specify ingress addresses and ports in the installation plan. Do not copy the current machine's connection settings as defaults for new installations.

[Updates and overrides](../docs/UPDATES.md) · [Tests](TESTING.md) · [Architecture](PLATFORM.md)
