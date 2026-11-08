# Loom installation kit — give this file to your agent

This kit installs Loom on a new Linux machine. It is not an upgrade or research-data migration tool. manifest.json records the service version and image architecture. Ubuntu/Debian hosts with systemd are the primary target.

## Decisions to establish with the user

1. Run `python3 setup.py inspect` to inspect OS, architecture, tools, block devices and existing installations. Check available facts before asking about unknown choices.
2. Establish CPU-only versus GPU Job requirements, data location, existing volume selection, editable dependency source root (default `/dependencies`), repositories and pinned commits. Research frameworks are external Job dependencies, not Loom prerequisites.
3. If using Tailscale, the user authenticates enrollment into the intended account/network. Never record authentication tokens or private keys in chat or installation plans.
4. Copy `install-plan.example.json` to `install-plan.json`, apply the choices and explain the changes. Once installation is authorized, do not request approval again for ordinary steps. Disk formatting is unsupported.

## Host preparation

Required tools: Python 3, Docker Engine and Compose v2, OpenSSL, acl (setfacl/getfacl), util-linux (findmnt/lsblk/blkid/mount) and systemd. Git is needed for source clones. A tailnet plan handles Tailscale installation, login and approval wait/resume. Administrator browser login and policy approval require actual user input. GPU execution additionally requires compatible host NVIDIA drivers and NVIDIA Container Toolkit.

Install missing packages according to the detected OS and existing installation. Do not replace an existing Docker distribution indiscriminately; consult distribution/official installation documentation. The kit does not contain Linux package repositories, GPU drivers or CUDA/Sim/training images. Offline installations must import the required host packages and selected research images separately.

## Storage and external dependencies

- For an already mounted `/storage`, use `mount: null` and `data: /storage/loom/data`. Create only the new data subdirectory; do not chown the entire existing storage tree.
- To mount an **existing filesystem**, set `mount` to `{"uuid":"actual-UUID","target":"/storage","fstype":"ext4"}`. xfs is also supported. Verify the UUID and contents identify the intended volume. The installer never formats disks and rejects duplicate mounts, fstab conflicts and nonempty mountpoints.
- The installer creates the dependency root and `installations/` without overwriting existing contents or ownership. Declare selected sources in `repositories`, for example `{"name":"example-library","url":"https://example.com/repo.git","commit":"40-character commit SHA"}`. Only selected sources are cloned, submodules initialized and actual commits recorded. Arrange private repository authentication separately.
- This is **host source provisioning**, not bulk installation of host Python environments. Keep research patches, Dockerfiles and lockfiles in `installations/<dependency>/<revision>/`, build execution images and register Job profiles. Existing research environments are not cloned.

## Installation

```bash
python3 setup.py inspect
python3 setup.py check install-plan.json
sudo python3 setup.py apply install-plan.json
```

`check` validates the plan and kit integrity. `apply` additionally checks host prerequisites, then loads images, optionally mounts a volume, provisions dependency source, creates the loom account/host layout, attaches executor/observation services, starts Compose and verifies API/MCP. It does not run installation scripts from user-selected repositories as root.

The non-login `loom:loom` account receives no sudo or Docker group privileges. Services run as non-root; the host executor responsible for Docker execution is managed separately. Authentication is enabled by default.

After reboot, `loom-stack.service` waits for data mounts before starting Compose. `install-progress.json` records progress. On failure, inspect the last stage and remaining paths before recovery. Do not automatically delete existing installations or blindly repeat apply.

## Startup verification and user handoff

`python3 setup.py verify` checks local API, Dashboard, MCP and executor connectivity. When needed, verify real execution with a user-authorized small Job. For GPU installations, verify actual GPU visibility inside a container before research submission.

On hosts without existing Serve configuration, selecting Tailscale configures HTTPS Dashboard `/`, Ops `/api`, MCP `/mcp` and Dashboard-specific relay routes. The base installation does not enable Funnel/public Internet exposure. Check the final URL with `tailscale serve status` and connect from another authorized Tailscale client.

Tokens live at `/loom/ops/secrets/client-tokens.json`. Deliver them through a separate secure channel, never a shared kit. The bundled `loom-agent-kit.zip` contains usage guides; after connecting, each user can download `/api/guide/bundle` under their own permissions. Do not share the installer's owner token with every user.

## Kit and host layout

```text
installation-kit/
├── INSTALL.md
├── setup.py / install-plan.example.json
├── manifest.json                  # File SHA256, image IDs and architecture
├── images.tar                     # Offline kits only; source installs reuse local images
├── loom-agent-kit.zip              # Guides, MCP reference and optional entry skill
└── platform/                      # Version-matched Compose and host installers
    ├── deploy/
    ├── ops/
    └── executor/

installed-host/
├── loom/
│   ├── ops/                       # New configuration, DB, logs and credentials
│   ├── components/loom/           # User component space
│   ├── profiles/loom/             # Execution profiles
│   ├── shared/                    # Executor and observations
│   ├── loom_use/<version>/        # Usage bundle
│   └── distribution/<version>/    # Compose and host installers for restart
├── dependencies/                  # Selected external source and installation recipes
└── storage/loom/data/              # New artifact storage on the selected volume
```

**Images are not copies of host `/loom`.** Code, libraries and default guides belong in images. Accounts, environment configuration, secrets, databases and storage connections belong to host installation. The kit excludes existing DBs, data, models, tokens and machine-specific entry skills.

## Tailscale and Dashboard SSO

The `tailscale` field in `install-plan.example.json` accepts the object from `tailnet-plan.example.json` instead of a boolean. `tailnet` is the expected organization; `users` optionally maps existing actors; `sso` defines native user bindings and local sessions. Reference protected files for management credentials instead of embedding secret values in plans.

Incomplete enrollment returns `waiting_for_tailnet` before host installation and records progress. After administrator login/approval, rerun the same command. Resume networking with `platform/deploy/tailnet.py`. For existing instances, apply only the relevant stage rather than rerunning the new-installation tool.

Read the repository [detailed procedure](../../docs/TAILSCALE-SSO.md); kits also include platform/docs/TAILSCALE-SSO.md. Merge policies within the organization's authorized scope.

## Optional access requests

For users outside the host tailnet, prepare access-plan.example.json and set the tailnet plan's `access_plan` to its absolute path. Default intake uses email; no separate Google login is required. Register and validate the management API credential in a protected file during setup. This credential is separate from tailscale up login and must not appear in chat or Git.

Setup validates management credentials, attaches a separate public listener and exposes only that port through Funnel. Private Dashboard/MCP remain private. See platform/docs/ACCESS-ENROLLMENT.md for steps and required permissions.
