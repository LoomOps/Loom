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

Installation handoff offers Loom Use for the current agent and user, then delegates
to the exact external release INSTALL.md. It never embeds client setup logic.
`loom_use/client-release.json` declares version/URL/checksum. `deploy/client_release.py`
acquires that package without installing a Skill or touching user configuration.

Server guides and generated schemas remain in Loom. The actual client lives in
LoomOps/Loom-Use. The manifest exports client_release alongside existing revisions.
Loom owns update-impact review and the pin; the client owns checking/applying it.
No compatibility-range solver or automatic background updater is introduced.

Tests/ops/client_source.py loads the exact checksum-verified package, not a sibling
checkout. HTTP integration covers initialization, token rejection/revocation, discovery,
status, guide and old-config migration. CI prepares the release before tests.
A small server Skill bridge retains old /loom/loom_use/current links; new links target
user-installed release directories. No independent client code is vendored here.
