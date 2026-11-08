# Install Loom on a new machine

Give an agent this repository and ask it to **install Loom using this document**. The installer is `deploy/installer/setup.py`: inspect the host first and ask only for the choices the installation needs. If Loom is already installed, follow [the update procedure](docs/UPDATES.md) instead of running a fresh installation.

## 1. Repository access and release pinning

If the repository is private, the installer needs GitHub authentication and read access. Public repositories can be cloned without a token. Never put credentials in chat or an installation plan.

```bash
git clone --branch v0.1.0-alpha.16 --depth 1 https://github.com/LoomOps/loom.git
cd loom
python3 deploy/installer/setup.py inspect
```

For an existing clone, use a separate checkout/worktree pinned to the release tag. Check GitHub Releases for a newer published version and change both the tag and version argument below together. Unpublished changes on `main` are not a release.

## 2. Inspect the environment and installation choices

- Use an Ubuntu/Debian systemd host with Python 3, Docker Engine and Compose v2, OpenSSL, acl and util-linux. Install missing tools for the actual OS.
- Choose CPU/GPU support, artifact storage, any existing volume and an optional dependency directory (default `/dependencies`). GPU drivers and NVIDIA Container Toolkit are separate host prerequisites.
- The user signs in to the intended Tailscale tailnet. The optional Sharing request service also needs management API credentials in a protected file. GitHub and Tailscale authentication are independent.
- The first build needs internet access for base images and Python dependencies. Air-gapped installation requires an [offline kit](deploy/README.md) and separately prepared host packages.

Read the [script behavior, plan fields and permissions](deploy/installer/INSTALL.md) before proceeding. The installer does not format disks or recursively change ownership of existing data.

## 3. Build an installation kit from source

Choose a new, empty staging destination. Building does not require an existing `/loom`. Images are built on the host; publishing to a registry or downloading image tar files is not required for source installation.

```bash
sudo python3 deploy/source_install.py 0.1.0-alpha.16 --destination /var/tmp/loom-alpha15-install
```

This produces release source, images, guidance and an integrity manifest; it does not install services yet. Copy the generated `installer/install-plan.example.json` to a separate plan and enter the **actual host's choices**. Do not use example paths or accounts unchanged.

```bash
cd /var/tmp/loom-alpha15-install/installer
python3 setup.py check /absolute/install-plan.json
sudo python3 setup.py apply /absolute/install-plan.json
sudo python3 setup.py verify
```

`apply` creates the `loom:loom` service identity, base `/loom` layout, selected storage connection, host executor and Compose services. When Tailscale enrollment is needed, installation state is preserved while the user signs in. It does not clone another machine's database, keys or research environment.

## 4. Verify installation

Report startup checks and actual Dashboard/MCP addresses. With an authorized small CPU Job, verify submission, execution and artifact retrieval. For GPU installations, also verify GPU exposure inside a container. Reboot and external-client access must be checked on that machine separately.

Users start with [`loom_use/`](loom_use/README.md) and obtain the current version/contracts from `system.status`, `system.guide` and `system.describe`. Do not share the installer's token with every user.

## Release discovery configuration

Ops checks GitHub Releases every six hours; it never installs automatically. Public repositories need no GitHub token. Private repositories need a server-side Contents:read token. The local GitHub login used for cloning is not automatically copied into the container.

```bash
# Only when the release repository requires authentication
sudo python3 deploy/release_check_setup.py --root /loom/ops --channel alpha --credential
```

Configure this before first startup or at the next safe Ops restart for an existing service. `--disable` supports offline installations. The same setup helper is included in the source/offline installation kit. See the [release discovery contract](docs/UPDATES.md#new-release-discovery); check the Dashboard footer or MCP `system.updates` for the actual result.
