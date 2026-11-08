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
  --release /loom/builds/alpha/0.1.0-alpha.16 \
  --output /path/to/new/loom-install-kit \
  --guide /path/to/loom-agent-kit-0.1.0-alpha.16.zip
```

Give the resulting archive to the target user's agent with instructions to read
`INSTALL.md`. The agent inspects the host, discusses storage/dependency/GPU needs,
and applies the agreed plan using `setup.py`. The installer supports existing
filesystem UUID mounts without formatting, optional pinned Git sources under
`/dependencies`, and a systemd Compose unit that waits for storage mounts.
Host OS packages, GPU drivers and optional research environments are separate.
This is a new-machine installation path; it refuses existing Loom instances.

## Low-level host setup (advanced)

For the complete new-machine workflow and boot service, use the installer above.
These lower-level commands are for deliberate manual setups.
Install Docker Engine/Compose, Python 3, OpenSSL and `acl` (`setfacl`/`getfacl`).
GPU Jobs additionally need the host NVIDIA driver and Container Toolkit.
Load or build matching versioned Ops/Dashboard images, then run (replace the example tags with the intended release):

```bash
sudo python3 deploy/install.py --data /storage/loom/data \
  --project loom --api-port 18892 --dashboard-port 18893 --tls-port 18894 \
  --ops-image loom-ops:0.1.0-alpha.16 \
  --dashboard-image loom-dashboard:0.1.0-alpha.16
sudo python3 deploy/executor-attach.py /loom/ops --instance alpha \
  --data /storage/loom/data --install-service --gpu
LOOM_INSTANCE=/loom/ops LOOM_PROJECT=loom bash deploy/compose.sh up -d --no-build
```

The default instance root is `/loom/ops`. Neither instance nor data directory may
already exist. The installer creates `loom` only if missing and reuses its existing
numeric UID/GID. It refuses an inconsistent existing account. Advanced installations
may explicitly supply both `--uid` and `--gid`; that opt-out does not provision the
managed host account or observation service. Use a real non-root identity.

Account creation is host-side, never inside a privileged Compose bootstrap container.
Compose uses the generated numeric UID/GID. `compose.sh` obtains local administrative
access when needed to read service-owned configuration; containers still run as loom.
Keep service token files mode 0600 and secrets directories 0700. Operators can read
credentials with `sudo cat /loom/ops/secrets/client-tokens.json`.

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
