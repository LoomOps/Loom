# Identity and authorization — detailed design

Status: proposed Rust redesign; no implemented/deployed claim. Phases: D1, D6.

## Responsibility and model

**Authority:** loom-auth policy with loom-app authorization; retained web/provider adapters.

**Typed model:** PrincipalId, CredentialId, ScopeSet, Delegation, BrowserAssertion, InstallationEpoch.

**Operations:** AuthenticateCredential; AuthorizeOperation; RotateOrRevokeCredential. Queries: current principal, grants and scoped action discovery.

**Persistence:** Existing user_profiles/auth_identities/credentials/resident_tokens and their current hashes/expiry/revocation semantics; browser sessions remain with the retained web owner.

## Reference source and consumers

- [dashboard/requirements.txt](../../../dashboard/requirements.txt)
- [dashboard/sso.py](../../../dashboard/sso.py)
- [ops/access.py](../../../ops/access.py)
- [ops/ownership.py](../../../ops/ownership.py)
- [ops/service_tokens.py](../../../ops/service_tokens.py)

Existing feature dependencies: none declared.
The references establish old behavior, not target Rust module boundaries.

## Implementation work

1. Capture bearer, trusted Dashboard assertion and Tailscale actor fixtures independently. The transport cannot synthesize a Tailscale person from a tagged device or Linux username.

2. Pass a verified Principal/ScopeSet into each application command. Re-read revocation/delegation at mutation commit; never authorize by a caller-supplied owner_kind or Built-in label.

3. Keep namespace, origin, display attribution and shared usability separate. Preserve existing shared-software semantics and operation-specific grants; any tighter write policy requires an explicit public decision.

4. Use constant-time secret comparison and existing token hash/issuer namespaces. Preserve current credentials through the Rust cutover; language change is not token rotation.

5. Keep browser cookie/CSRF/session invalidation in its existing adapter until independently migrated. An API framework middleware is not the complete authentication design.

## Transaction and effect boundary

Credential/grant decisions and the affected mutation share an authoritative snapshot/transaction or an explicit revocation-version fence. No network identity lookup is held inside a write transaction.

## Retained code and compatibility

Existing SSO/Sharing/Tailscale SDK and browser-session code, plus private credentials. Do not import the entire web identity implementation into Rust just to increase its language share.

Preserve identity tables/secrets; verify hashes/IDs before and after cutover. Session/provider migrations remain a separate retained-boundary change.

## Acceptance cases

| Case | Trigger | Required result |
|---|---|---|
| ID-01 | Revoked token races Job submit | Commit rejects the revoked grant; no accepted Job or launch intent. |
| ID-02 | Display owner/origin changes | Exact software namespace/pins and write permissions remain unchanged. |
| ID-03 | Tagged resident calls browser-only identity flow | No fabricated human identity or token-only installation success. |
| ID-04 | Existing credential after migration | Same authenticated principal/scopes; no duplicate account. |

Reference regression modules (adapt moved internals to actual Rust boundaries):

- [tests/ops/test_access_contract.py](../../../tests/ops/test_access_contract.py)
- [tests/ops/test_shared_ownership.py](../../../tests/ops/test_shared_ownership.py)
- [tests/ops/test_dashboard_identity.py](../../../tests/ops/test_dashboard_identity.py)
- [tests/dashboard/test_sso.py](../../../tests/dashboard/test_sso.py)
- [tests/ops/test_service_tokens.py](../../../tests/ops/test_service_tokens.py)

## Exit review

Validate the cases above plus the shared [domain](../DOMAIN.md), [API](../API-CHANGES.md),
[compatibility](../COMPATIBILITY.md) and [migration](../MIGRATION.md) contracts.
Record exact source/artifact hashes and which external boundaries remain unverified.
A passing old Python suite or compiling crate alone does not close this block.
