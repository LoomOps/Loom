# onboarding — User guides, version identification and client entry

## Responsibilities and boundaries

The running server is authoritative for guides. Updates do not change personal instructions or active Job pins. Distributable guides, document titles and generated MCP/API reference descriptions use English; Dashboard UI localization remains independent.

## Inputs, outputs and authoritative sources

- [ops/onboarding.py](../../ops/onboarding.py)
- [loom_use/entry.md](../../loom_use/entry.md)

The linked code/schemas define fields, enums and state transitions. This document does not duplicate them.

## Call contracts

| Boundary | Input | Result or failure | Consumers |
|---|---|---|---|
| `ops/onboarding.py#bundle` | Actor, client-known revision, current schemas/contracts | Version-bound guide package and update identity; no automatic credential issuance | api, installation |

## Related features and change impact

[identity](identity.md) · [api](api.md)

When usage contracts change, update Loom Use documents, bundle inventory, revisions and the stdio client.

## Tests and failure boundaries

- [tests/ops/test_onboarding.py](../../tests/ops/test_onboarding.py)

Test sources define the concrete scenarios. `deploy/test.sh` records individual results as JSON/JUnit.

## Live validation

Verify actual client reconnection and guide refresh separately.

Installation handoff explicitly offers optional Loom Use skill setup and MCP connection registration in the user's selected client/machine. The scripted handoff provides the private endpoint, bundle URL and verification actions, but never claims registration from server-side health. Existing client settings are preserved; actual client discovery/status/guide checks are reported separately from server readiness.

Resident Codex clients use `loom_use/clients/setup.py configure` with a Dashboard Admin-issued permanent token. `doctor` verifies the protected credential, matching user configuration and four live MCP operations. The static headers helper never issues or renews credentials. Installation docs must distinguish server readiness, saved client settings and actual client discovery.

Guide refresh accepts explicit Tailscale identity or an Admin-issued token. It
verifies the current principal and includes identity/scopes in its cache key;
switching VPN users must not reuse another user's scoped bundle. Both the native
header helper and optional stdio transport use the same private-file reader.
