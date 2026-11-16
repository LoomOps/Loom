# dashboard — Dashboard HTTP relay·workspace UI

## Boundary

Never send credentials to the opaque iframe. Browser writes respect Origin/CSRF boundaries. The UI must not infer completed backend states.

The current native service is implemented in `rust/`. Python files in this block may be retained web/administrator integrations or compatibility references; they do not supersede the native transactional handlers. See [implementation boundaries](../IMPLEMENTATION.md).

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Browser relay | `dashboard/server.py#WebHandler` | Allowed read route or queue mutation + authenticated browser | Bounded response; session/Origin/CSRF guard for writes; no generic arbitrary proxy | api |
| HTTP server | `dashboard/server.py#make_server` | Ops origin, optional SSO config and exact trusted ingress | Dashboard server with explicit authentication mode | installation |

## Sources and contracts

- [dashboard/.dockerignore](../../dashboard/.dockerignore) — integration, packaging or documentation.
- [dashboard/Dockerfile](../../dashboard/Dockerfile) — integration, packaging or documentation.
- [dashboard/branding/brand.css](../../dashboard/branding/brand.css) — integration, packaging or documentation.
- [dashboard/branding/brand.js](../../dashboard/branding/brand.js) — integration, packaging or documentation.
- [dashboard/branding/logo.svg](../../dashboard/branding/logo.svg) — integration, packaging or documentation.
- [dashboard/branding/scene.js](../../dashboard/branding/scene.js) — integration, packaging or documentation.
- [dashboard/dev.py](../../dashboard/dev.py) — integration, packaging or documentation.
- [dashboard/server.py](../../dashboard/server.py) — integration, packaging or documentation.
- [dashboard/theme/style.css](../../dashboard/theme/style.css) — integration, packaging or documentation.
- [dashboard/theme/theme.js](../../dashboard/theme/theme.js) — integration, packaging or documentation.
- [dashboard/workspace/access.html](../../dashboard/workspace/access.html) — integration, packaging or documentation.
- [dashboard/workspace/activity.js](../../dashboard/workspace/activity.js) — integration, packaging or documentation.
- [dashboard/workspace/build.py](../../dashboard/workspace/build.py) — integration, packaging or documentation.
- [dashboard/workspace/dependencies.js](../../dashboard/workspace/dependencies.js) — integration, packaging or documentation.
- [dashboard/workspace/details.js](../../dashboard/workspace/details.js) — integration, packaging or documentation.
- [dashboard/workspace/explorer.js](../../dashboard/workspace/explorer.js) — integration, packaging or documentation.
- [dashboard/workspace/extensions.js](../../dashboard/workspace/extensions.js) — integration, packaging or documentation.
- [dashboard/workspace/frame.css](../../dashboard/workspace/frame.css) — integration, packaging or documentation.
- [dashboard/workspace/frame.js](../../dashboard/workspace/frame.js) — integration, packaging or documentation.
- [dashboard/workspace/icons.js](../../dashboard/workspace/icons.js) — integration, packaging or documentation.
- [dashboard/workspace/job-activity.js](../../dashboard/workspace/job-activity.js) — integration, packaging or documentation.
- [dashboard/workspace/lazy_logs.py](../../dashboard/workspace/lazy_logs.py) — integration, packaging or documentation.
- [dashboard/workspace/mcp.js](../../dashboard/workspace/mcp.js) — integration, packaging or documentation.
- [dashboard/workspace/media.js](../../dashboard/workspace/media.js) — integration, packaging or documentation.
- [dashboard/workspace/operations-desk.css](../../dashboard/workspace/operations-desk.css) — integration, packaging or documentation.
- [dashboard/workspace/parent.css](../../dashboard/workspace/parent.css) — integration, packaging or documentation.
- [dashboard/workspace/parent.js](../../dashboard/workspace/parent.js) — integration, packaging or documentation.
- [dashboard/workspace/projection.js](../../dashboard/workspace/projection.js) — integration, packaging or documentation.
- [dashboard/workspace/queue.js](../../dashboard/workspace/queue.js) — integration, packaging or documentation.
- [dashboard/workspace/research.js](../../dashboard/workspace/research.js) — integration, packaging or documentation.
- [dashboard/workspace/surfaces.js](../../dashboard/workspace/surfaces.js) — integration, packaging or documentation.
- [dashboard/workspace/template.html](../../dashboard/workspace/template.html) — integration, packaging or documentation.
- [dashboard/workspace/text-preview.js](../../dashboard/workspace/text-preview.js) — integration, packaging or documentation.
- [dashboard/workspace/search.js](../../dashboard/workspace/search.js) — integration, packaging or documentation.
- [dashboard/workspace/catalog-pages.js](../../dashboard/workspace/catalog-pages.js) — integration, packaging or documentation.

## Change review

Rebuild the generated workspace after UI source edits. Verify both HTTP behavior and real Firefox for authentication, session or relay changes. Review the full installation doctor and step exit criteria when this feature changes; a local health response does not establish user-facing readiness.

The native resource response preserves the workspace's `host`, `observed_at` and
`sections` envelope. Host labels come from the recorded host observation, with
the installation machine ID retained separately and stale/missing samples marked.
Verify Models, Datasets, Experiments and Troubleshooting beyond the bootstrap and
first page; Models and Experiments have independent list renderers.

Related blocks: [identity](identity.md) · [api](api.md) · [catalog](catalog.md) · [observations](observations.md) · [onboarding](onboarding.md) · [diagnostics](diagnostics.md).

## Verification

- [tests/dashboard/browser/test_workspace.py](../../tests/dashboard/browser/test_workspace.py)
- [tests/dashboard/test_proxy.py](../../tests/dashboard/test_proxy.py)

Run the affected suites and `deploy/features.py check`; see [the verification matrix](VERIFICATION.md). Passing retained compatibility fixtures is distinct from exercising the native handler.

Acceptance outside these fixtures:
- Actual Safari behavior, existing user sessions and SSO provider login require separate acceptance testing.


## Bounded catalogs and refresh

A bootstrap is only a seed. Same-revision refresh preserves loaded list pages and
parent-side detail identities. A new revision invalidates page cursors and pending
responses before the visible list reloads. Research history uses its selected
scope; current queue occupants remain visible in every presentation view.
Tasks appear as filters and record labels, without a standalone Scenario tab.
Explorer requests actual storage observations even with the Inspector closed and
loads registered file records independently of the research pane. Its bounded
pages are not a recursive filesystem inventory or an assertion that unregistered
files do not exist.
