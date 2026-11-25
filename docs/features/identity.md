# identity — User authentication, SSO sessions and current ownership

## Boundary

The principal's `read` scope controls shared discovery. Mutation authority is
checked by each action, including owner and administrator checks where required.
Browser sessions and MCP authentication are independent. Invalidated browser
sessions cannot be reused.
Signed Dashboard claims and trusted Tailscale ingress resolve configured user bindings with the same Unicode casefolding. A change of transport cannot select a different principal solely through email casing.
Relay trust keys are bounded regular files; unavailable or special-file keys fail authentication without waiting for a writer.

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Browser authentication | `dashboard/sso.py#SSO` | Trusted Tailscale ingress + native user binding or ready machine share | Verified session or rejected login; no provider token in browser | dashboard, network |
| Caller identity | `rust/crates/core/src/auth.rs#authenticate` | Signed, expiring Dashboard identity | Shared actor with server-selected scopes; invalid/expired assertions rejected | api |
| Current responsibility | `rust/crates/core/src/composition.rs#assign_responsibility` | Item identity + registered profile + actor | Owner overlay and audit row; original item preserved | results, catalog |
| Resident agent tokens | `rust/crates/core/src/http.rs#admin_tokens` | Signed Dashboard Admin claim, registered user and allowed scopes | One-time permanent token; only digest and revocation metadata persisted | dashboard, api, onboarding, installation |

## Sources and contracts

- [dashboard/requirements.txt](../../dashboard/requirements.txt) — integration, packaging or documentation.
- [dashboard/sso.py](../../dashboard/sso.py) — integration, packaging or documentation.
- [deploy/generate_identity_codec.py](../../deploy/generate_identity_codec.py) — integration, packaging or documentation.
- [ops/access.py](../../ops/access.py) — retained Python compatibility reference.
- [ops/ownership.py](../../ops/ownership.py) — retained administrator attribution adapter used by assign-owner.py.
- [ops/service_tokens.py](../../ops/service_tokens.py) — retained Python compatibility reference.
- [rust/crates/core/src/auth.rs](../../rust/crates/core/src/auth.rs) — native implementation / contract.
- [rust/crates/core/src/casefold.rs](../../rust/crates/core/src/casefold.rs) — native implementation / contract.
- [loom_use/operator.md](../../loom_use/operator.md) — integration, packaging or documentation.

## Change review

When authentication or profile contracts change, review Ops actors, the Dashboard relay, new installations, ownership display and user guides together. Review the full installation doctor and step exit criteria when this feature changes; a local health response does not establish user-facing readiness.

## Verification

- [tests/dashboard/test_sso.py](../../tests/dashboard/test_sso.py)
- [tests/ops/test_access_contract.py](../../tests/ops/test_access_contract.py)
- [tests/ops/test_dashboard_identity.py](../../tests/ops/test_dashboard_identity.py)
- [tests/ops/test_service_tokens.py](../../tests/ops/test_service_tokens.py)
- [tests/ops/test_shared_ownership.py](../../tests/ops/test_shared_ownership.py)
- [tests/rust/application.rs](../../tests/rust/application.rs)

Acceptance outside these fixtures:
- Verify actual Tailscale clients, Sharing revocation and TLS separately.
