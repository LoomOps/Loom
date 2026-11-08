# Loom

Loom lets multiple users and clients queue CPU/GPU Jobs and manage execution state, logs and artifacts. Callers declare preparation, execution, completion and error-handling actions; Loom executes and records them. Error diagnosis, fixes, event consumption and agent operations remain the caller's responsibility.

## Installation

This repository includes source, Dockerfiles, Compose definitions and host installation scripts. Ops and Dashboard run in containers; a host executor launches Job containers. The host `/loom` directory stores configuration, credentials and databases. Artifact storage and optional external dependencies are connected separately.

- **[Install on a new machine: INSTALL.md](INSTALL.md)**
- [Installation and operations](deploy/README.md)
- [Agent-assisted installation and script reference](deploy/installer/INSTALL.md)

Pin a [GitHub Release](https://github.com/LoomOps/loom/releases) tag to build images and an installation kit from source. A separate image registry is not required. Private repositories require GitHub read access; host administration and Tailscale enrollment are separate permissions.

## Usage

**MCP guidance, client tools and optional skills live in [`loom_use/`](loom_use/README.md).**

Connect to `<Loom URL>/mcp` using the server address and your own authentication. Use `system.status`, `system.guide` and `system.describe` to discover the running version, usage guidance and exact input contracts. Dashboard is the web entry point for the same server.

Loom does not embed resident agents or a prescribed agent hierarchy. External operators are MCP/API clients. They own machine-specific instructions, agent assignment and startup policies.

## Repository layout

```text
ops/          Jobs, queues, results and MCP/API server
dashboard/    Web UI
executor/     Host container executor
loom_use/     Client tools, MCP guidance, skills and optional adapters
deploy/       Installation, Compose, builds and releases
tests/        Ops, Dashboard, browser and installation tests; shared runner
docs/         Platform technical documentation
components/   Component documentation
```

## Development and validation

[Feature specifications](docs/features/README.md) describe responsibilities, source ownership, contracts, dependencies, change impact and tests.
Use [authentication design and setup](docs/TAILSCALE-SSO.md) for Tailscale enrollment and Dashboard authentication.

Development and production use separate databases, storage and ports. Validate local edits in an isolated dev-build; production updates are separate deployments. Follow local machine instructions for actual paths and connection settings.

```bash
# Unit, integration and packaging tests
python3 tests/run.py

# Include Firefox UI validation
python3 tests/run.py --browser
```

In an existing development installation, `deploy/build.sh` creates numbered images and `deploy/compose.sh up -d --no-build` applies them. Restart Ops after active Jobs return their resources. On a new machine, complete installation first.

[Testing](deploy/TESTING.md) · [Platform architecture](deploy/PLATFORM.md) · [Documentation index](docs/README.md)

## Installation and updates

[Structured installation](deploy/installer/INSTALL.md) · [Source installation, overrides, updates and rollback](docs/UPDATES.md)

Client tools and guidance live in `loom_use`; development specifications live in `docs/features`. Machine-specific instructions, secrets, research data and historical execution evidence are not part of the Git distribution.

## License

Loom is licensed under [GNU GPL version 3 only](LICENSE) (`GPL-3.0-only`).
See [third-party notices](THIRD_PARTY_NOTICES.md) for dependencies and trademarks.

For service removal and machine access cleanup, see [Uninstall](docs/UNINSTALL.md).
