# Loom development

- This repository owns platform source, not host state. `/loom` contains persistent installation data; never commit credentials, runtime DBs, data, local overrides or machine-specific instructions.
- Use an isolated dev-build instance by default. Instance paths and project names are selected with `LOOM_INSTANCE` and `LOOM_PROJECT`; `deploy/compose.sh` defaults to `/loom/ops/dev-build` / `loom-dev-build`.
- Dev, alpha and remote installations are distinct targets. Do not deploy local edits to another target implicitly. Keep UI design and API contracts unless the user requests a change.
- UI source edits rebuild the dev browser without an image build. Restart Ops only after actual execution and result/callback work returns.
- Immutable release images capture source and Git provenance. Never change an existing numbered release. Authentication bypass is dev-only.
- Read `docs/features/README.md` and the relevant feature spec. `catalog.json` maps source, contracts, consumers and tests.
- Update API/MCP, UI consumers, loom_use and installation packaging together when their contract changes. Run `deploy/features.py check`, impact and appropriate regression tests.
- Generated Dashboard HTML comes from workspace sources. `template.html` is a hash-pinned empty-data template: never embed host catalogs, operational instructions or measured research snapshots. If explicitly changing its structure, update the pinned hash, rebuild and test the generated workspace.
- Runtime extensions/profiles under `/loom` are user-owned configuration. Platform upgrades preserve them; schema incompatibilities must be reported, never silently rewritten.
- Feature checks are development tools, not additional Job execution gates. No internal agent hierarchy is required.
- For a new-machine installation, start with root `INSTALL.md`; existing installations follow `docs/UPDATES.md`. Do not infer local machine credentials or storage choices from this checkout.
- Keep test suites and fixtures under `tests/`; use `python3 tests/run.py` (`--browser` for Firefox). Test evidence is not release source.

- Write distributable instructions, README files, developer specifications and MCP/API reference descriptions in English. Preserve intentional UI localization and multilingual test fixtures.
