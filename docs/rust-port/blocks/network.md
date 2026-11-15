# Networking, Sharing and provider boundaries — detailed design

Status: implemented development candidate; operational deployment not performed.
Concrete ownership, test boundaries and external limits: [IMPLEMENTATION](../IMPLEMENTATION.md). Phases: D1, D6, D7.

## Responsibility and model

**Authority:** Retained Gateway/Sharing/Tailscale integration with Rust Ops typed endpoints.

**Typed model:** ServerOrigin, TrustedProxy, ShareGrant, UpstreamAvailability and RequestIdentity.

**Operations:** Existing authenticated route forwarding/sharing lifecycle; Rust receives verified identity context only from configured trusted boundaries.

**Persistence:** Existing private provider/share request and managed-resource journals; no migration of external rights based on display metadata.

## Reference source and consumers

- [deploy/tailnet.py](../../../deploy/tailnet.py)
- [dashboard/admission.py](../../../dashboard/admission.py)
- [dashboard/admission_http.py](../../../dashboard/admission_http.py)
- [deploy/access_setup.py](../../../deploy/access_setup.py)
- [deploy/installer/access-plan.example.json](../../../deploy/installer/access-plan.example.json)
- [dashboard/gateway.py](../../../dashboard/gateway.py)
- [dashboard/gateway.js](../../../dashboard/gateway.js)
- [dashboard/gateway.html](../../../dashboard/gateway.html)
- [dashboard/gateway.css](../../../dashboard/gateway.css)
- [dashboard/sharing.py](../../../dashboard/sharing.py)
- [dashboard/managed_resources.py](../../../dashboard/managed_resources.py)

Existing feature dependencies: identity, diagnostics.
The references establish old behavior, not target Rust module boundaries.

## Implementation work

1. Freeze trusted assertions, TLS, Origin, upstream allowlists and provider identity fixtures before switching Ops.

2. Retain public request-only Gateway and separate private Admin services. Do not put a privileged Rust registrar on a public route.

3. Keep bounded HTTP timeouts/errors and exact reconnect/unknown-external-write reconciliation. A 503 must not erase a specific permissions/local-state fault in private diagnostics.

4. Preserve service-account readability on atomic managed-resource journal replacement: UID/GID, mode, no-symlink checks and directory durability.

5. Observe and report external approval/adoption state separately from local grant/read failures. No automatic network/account reset during a language cutover.

## Transaction and effect boundary

Provider calls occur outside short journal transactions and retain intent/receipt identities. Rust Ops trusts only verified scope/issuer context, never arbitrary forwarded headers.

## Retained code and compatibility

Web/network/provider integrations and existing host helpers. Rewriting these libraries offers no prerequisite benefit to the Job domain redesign.

Preserve private share/token/journal state and external identities independently per installation. Real provider approval/TLS/user access is separate from mocked tests.

## Acceptance cases

| Case | Trigger | Required result |
|---|---|---|
| NET-01 | Local management journal unreadable | Specific private diagnosis; no fabricated provider outage/approval state. |
| NET-02 | Forged identity/proxy header | Rejected outside configured trust boundary. |
| NET-03 | Share reply lost after upstream acceptance | Reconcile same intent; no duplicate grant or automatic deny. |

Reference regression modules (adapt moved internals to actual Rust boundaries):

- [tests/deploy/test_tailnet.py](../../../tests/deploy/test_tailnet.py)
- [tests/dashboard/test_admission.py](../../../tests/dashboard/test_admission.py)
- [tests/deploy/test_access_setup.py](../../../tests/deploy/test_access_setup.py)
- [tests/dashboard/test_gateway.py](../../../tests/dashboard/test_gateway.py)
- [tests/dashboard/test_sharing.py](../../../tests/dashboard/test_sharing.py)
- [tests/deploy/test_managed_resources.py](../../../tests/deploy/test_managed_resources.py)
- [tests/dashboard/test_managed_permissions.py](../../../tests/dashboard/test_managed_permissions.py)
- [tests/deploy/test_boot_readiness.py](../../../tests/deploy/test_boot_readiness.py)

## Exit review

Validate the cases above plus the shared [domain](../DOMAIN.md), [API](../API-CHANGES.md),
[compatibility](../COMPATIBILITY.md) and [migration](../MIGRATION.md) contracts.
Record exact source/artifact hashes and which external boundaries remain unverified.
A passing old Python suite or compiling crate alone does not close this block.
