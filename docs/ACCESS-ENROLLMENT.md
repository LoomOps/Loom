# Machine Sharing requests and approval

New installations use `enrollment_mode: sharing`. External users retain their personal tailnet and receive access to **only this Loom machine**. The flow does not call tailnet enrollment, user approval or device registration APIs. `tag:loom` is for Hub discovery, not user authorization.

## User and administrator flow

1. Enter only an email address on the public Funnel gateway. No device name, personal name or purpose is requested.
2. The server emails a single-use invitation for this machine, without exit-node access.
3. The user accepts through Tailscale. The mailbox and actual accepting account may differ.
4. Store `acceptedBy.id/loginName` and enter **awaiting administrator approval**. Operational ports remain closed.
5. An administrator verifies the actual accepting account in Dashboard **Tailscale → Access Request** and approves it.
6. Add only that account to the machine-specific approval group and allow only explicit ports on this machine's IP.
7. The user connects to Dashboard/MCP from their own tailnet. No device selection or tailnet switching is required.

The requester must accept the share. An email address or receipt cookie proves neither Tailscale identity nor approval. Administrators cannot approve before acceptance. Revoke access if the accepted identity subsequently changes.

A previous browser session cannot be used when the current Tailscale account differs. Admin cookies are no exception. Apply the same check to Gateway, data APIs and administrative APIs. Approved shared users receive new sessions under their own identity. Reject account changes during login as well.

## UI and APIs

- The `Tailscale` menu and lower-left profile open the same private tab.
- Ordinary users see Account; only Admin sees Member/Access Request.
- Pin the authenticated current Admin above members. List only **users whose share acceptance, Loom approval and grant application are complete**. Do not query tailnet `/users`. The Admin row is separate from share records and has no share-revoke button. Roles display Admin/Member. Acceptance alone or externally created shares do not qualify. Access Request displays English Grant status labels. Revocation immediately removes the user from Member.
- Administrative APIs: `/admission/session`, `/admission/requests`, `/admission/members`, `/admission/decide` (approve/deny/retry/revoke/remove), `/admission/share-revoke` (this machine's share ID). Admin Remove deletes fully cleaned denied/revoked requests and receipt links without leaving a deleted-state row; administrative audit logs remain. It cannot delete pending, ready or partially cleaned requests. Administration is integrated into the Dashboard Tailscale view. The public gateway exposes no administrative API, MCP or Jobs.
- Public intake APIs return only request status linked to the caller's receipt, not invitation URLs or the actual accepting account. Sharing mode does not expose `/api/device`, `/api/device-name` or `/api/devices`.

## Authentication and revocation

Internal users use explicit native Tailscale bindings. External shared users require **trusted Serve login plus this machine's ready Sharing record**, through machine-specific access grants. Sharing sessions use `authentication: tailscale-sharing` and `subject: shared-user:<id>`, distinct from the existing `(issuer, subject)` Admin allowlist.

Signed Dashboard relay assertions retain the installation issuer/key but are verified locally. Every shared-session data request rechecks Serve identity and current approval. Copying a cookie to another user or revoking the share prevents data access through that session. MCP retains trusted Serve authentication, with network grants restricting the same ports.

Deny/Revoke first removes Loom-granted group access, then deletes the share invitation. Preserve group access when the same account still needs it through another approved share or preserved membership approval. Never delete other machines, tailnet users, recipient devices or administrator-created permissions. Direct console share deletion is detected on the next reconciliation.

## Deduplication, retries and preservation

SQLite `share_requests` and `share_receipts` allow one active request per email and installation. Multiple browsers submitting the same email do not send duplicate invitations. Preserve original membership requests, audits and policy backups; the Sharing worker does not replay them. Without a receipt cookie, returning users cannot be identified automatically. Reentering the same email reconnects to the existing request.

Public intake combines email input with a Tailscale-logo `Request access via Tailscale` button. While waiting for acceptance or after preparation failure, the original requester sees `Use another email`. It cancels the old request and returns to input. Preserve cancellation records, shown to administrators as `Cancelling` → `Cancelled by user`, distinguished by internal revocation state and `requester_cancel` audit entries.

While awaiting acceptance, display only the sender name/email resolved from the invitation's actual `sharerId`. Lookup failure does not block sharing. Distinguish preparation delay, uncertain delivery and failure; API failure does not prove an account is unregistered. After expiry/revocation, `Request access` permits a new request.

Origin/CSRF-protected `POST /api/request/reset` moves only the original requester's unapproved request into cancellation pending (revoking), rotates receipt subject/CSRF and permits a new email. The private worker retracts sent invitations. A later receipt connected by knowing the same email cannot cancel the original request; neither can this route cancel already approved access. If cancellation races with sending, use a late invitation ID for cleanup without reactivating the request.

Defaults: 20 new requests/hour globally, 200 active pending requests and three requests/email/day. Configure the first two through `sharing_hourly_limit` and `sharing_pending_limit`. After seven days, unapproved requests expire and shares are revoked. Public IP addresses are not identity or deduplication keys.

Before sending, check whether existing policy already permits shared recipients. Broad wildcard/shared grants stop automatic sending; never silently rewrite existing policy. Recheck the actual accepting account as well. Policy updates use validation and ETag/If-Match, preserving unrelated rules.

If the send response is lost, rediscover the invitation in the device invitation list. When delivery cannot be established, retain `invite_delivery_uncertain` instead of repeatedly emailing. There is no automatic bulk resend.

Schedule ordinary pending checks every 30 seconds and completed-share checks every 60 seconds, executed by the existing 15-second worker. Failures use exponential backoff and appear in administration. This consumes neither the Job queue nor an agent thread.

## Installation and API references

- In `deploy/installer/access-plan.example.json`, specify installation ID, device ID/IP, private and Funnel origins, Admin issuer/subject and management credential file.
- A **user API access token** is required. OAuth client_credentials tokens cannot create sharing invitations. Keep the credential only in a protected server file.
- Run `access_setup.py check/inspect`, then `configure --root ... --plan ...`, then apply Dashboard/Gateway Compose. `inspect` reads actual device invitations/policy and checks preapproval blocking without sending email.
- Use the existing installation procedure for `register-machine` Hub tags and the dedicated Funnel listener.
- Do not grant access to unrelated machines or all Loom-tagged machines. Preserve common `tag:loom`, machine tags and existing administrator access.

Official API: https://tailscale.com/api and DeviceInvites in https://api.tailscale.com/api/v2?outputOpenapiSchema=true . Routes: `GET/POST /device/{deviceId}/device-invites`, `DELETE /device-invites/{deviceInviteId}`. Sharing semantics: https://tailscale.com/docs/features/sharing .

## Verification and real-user acceptance

Use real SQLite/HTTP/Firefox and mocked external APIs to test email request → automatic invitation → acceptance by a different actual account → Admin approval → machine-specific grant → sharing session → revocation. Include duplicate/concurrent intake, lost send responses, restarts, expiry, policy conflicts, broad-grant blocking, external-share lookup/revocation and forged Serve headers/cookies.

Actual external-user email delivery, acceptance in a personal tailnet and Dashboard/MCP connectivity require separate acceptance. Do not report those steps complete based only on read APIs or mocks.

The Funnel gateway cannot read a public visitor's personal tailnet information. It redirects through normal login only after validating origin/source/nonce on the private `/gateway/probe` result. Approval records alone do not establish VPN connectivity or authentication.
