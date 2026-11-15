# Network — Item/index impact and detailed work

Status: proposed. Impact: **Associated — regression only**. No runtime change is made by this plan.

## Observed baseline

Existing native identity, Sharing, gateway/private route and ingress trust boundaries govern access. Index data must not bypass them.

Reviewed source:

- [dashboard/server.py](../../../dashboard/server.py)
- [dashboard/sso.py](../../../dashboard/sso.py)
- [dashboard/admission.py](../../../dashboard/admission.py)
- [dashboard/gateway.py](../../../dashboard/gateway.py)
- [deploy/tailnet.py](../../../deploy/tailnet.py)

## Implementation / consumer review

1. Reuse authenticated private routes and current trusted identity; no new public search service or unauthenticated file endpoint.
2. Verify cross-origin/CSRF/forwarding rules for added query/read actions and shared-account revocation during an active cursor.
3. Keep Sharing/Tailscale policy, invitations, ports and network credentials unchanged. No host/network changes are required to prepare this plan.
4. Test that safe index status does not leak another installation or restricted match through counts/timing-dependent error bodies.

## Invariants

- An Item ID, preview token or index URL is not a public bearer grant.
- No provider policy or account change caused by search/rebuild.
- No network fetch during text extraction.

## Planned acceptance and dependencies

Cases: `AUTH-01`, `AUTH-02`, `AUTH-03`, `UI-01`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [identity](identity.md), [api](api.md).
