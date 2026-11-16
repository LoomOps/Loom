# network — Tailscale enrollment, Serve and native identity configuration

## Boundary

New installations keep Dashboard and MCP on private Serve and require a separate public request-only Gateway. tag:loom supports Hub discovery; user grants apply per machine.
tag:loom is Hub discovery only; access approvals target one stable machine/device identity and exact addresses. Public request listener exposes no operational data or MCP.

The current native service is implemented in `rust/`. Python files in this block may be retained web/administrator integrations or compatibility references; they do not supersede the native transactional handlers. See [implementation boundaries](../IMPLEMENTATION.md).

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Enrollment | `deploy/tailnet.py#enrollment` | Expected tailnet, hostname/tag/SSH preferences | Ready or bounded login/approval-wait result; never switches an existing tailnet | installation |
| Instance binding | `deploy/tailnet.py#configure` | Instance root, validated plan, MagicDNS name | Backed-up API auth config and Compose overlay; no implicit restart | installation, identity |
| Serve binding | `deploy/tailnet.py#serve` | Instance ports and planned HTTPS routes | Existing nonconflicting routes retained; collisions fail before route mutation | installation |
| Machine access requests | `dashboard/admission.py#Admission` | Verified external identity, private admin decision, configured machine and management credential | Durable invitation/user approval/exact machine policy state; recoverable failures | identity, dashboard, installation |

## Sources and contracts

- [dashboard/admission.py](../../dashboard/admission.py) — integration, packaging or documentation.
- [dashboard/admission_http.py](../../dashboard/admission_http.py) — integration, packaging or documentation.
- [dashboard/gateway.css](../../dashboard/gateway.css) — integration, packaging or documentation.
- [dashboard/gateway.html](../../dashboard/gateway.html) — integration, packaging or documentation.
- [dashboard/gateway.js](../../dashboard/gateway.js) — integration, packaging or documentation.
- [dashboard/gateway.py](../../dashboard/gateway.py) — integration, packaging or documentation.
- [dashboard/managed_resources.py](../../dashboard/managed_resources.py) — integration, packaging or documentation.
- [dashboard/sharing.py](../../dashboard/sharing.py) — integration, packaging or documentation.
- [deploy/access_setup.py](../../deploy/access_setup.py) — integration, packaging or documentation.
- [deploy/installer/access-plan.example.json](../../deploy/installer/access-plan.example.json) — integration, packaging or documentation.
- [deploy/tailnet.py](../../deploy/tailnet.py) — integration, packaging or documentation.
- [deploy/installer/tailnet-plan.example.json](../../deploy/installer/tailnet-plan.example.json) — integration, packaging or documentation.
- [docs/ACCESS-ENROLLMENT.md](../../docs/ACCESS-ENROLLMENT.md) — integration, packaging or documentation.

## Change review

When enrollment or proxy behavior changes, verify installation plans, exact trusted peers, callback URLs, reruns/pending approvals, TLS and policy scope. Review the full installation doctor and step exit criteria when this feature changes; a local health response does not establish user-facing readiness.

Related blocks: [identity](identity.md) · [diagnostics](diagnostics.md).

## Verification

- [tests/dashboard/test_admission.py](../../tests/dashboard/test_admission.py)
- [tests/dashboard/test_gateway.py](../../tests/dashboard/test_gateway.py)
- [tests/dashboard/test_managed_permissions.py](../../tests/dashboard/test_managed_permissions.py)
- [tests/dashboard/test_sharing.py](../../tests/dashboard/test_sharing.py)
- [tests/deploy/test_access_setup.py](../../tests/deploy/test_access_setup.py)
- [tests/deploy/test_boot_readiness.py](../../tests/deploy/test_boot_readiness.py)
- [tests/deploy/test_managed_resources.py](../../tests/deploy/test_managed_resources.py)
- [tests/deploy/test_tailnet.py](../../tests/deploy/test_tailnet.py)

Run the affected suites and `deploy/features.py check`; see [the verification matrix](VERIFICATION.md). Passing retained compatibility fixtures is distinct from exercising the native handler.

Acceptance outside these fixtures:
- Passing mocks does not replace actual administrator login, device approval or policy permissions.

Browser search, Runtime and Scenario actions must use the private Dashboard
session/CSRF relay, including read operations transported as POST. The native
`/api/actions` and `/mcp` routes retain their direct-browser Origin rejection.
The browser-relay-routes-v1 migration adds only missing owned private prefixes
and refuses conflicting targets or a public private-ingress listener.
