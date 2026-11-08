# network — Tailscale enrollment, Serve and native identity configuration

## Responsibilities and boundaries

Network access and web login are distinct. Default installations do not enable Funnel. Optional access enrollment exposes only a separate public listener through Funnel. tag:loom is for Hub discovery; user approval targets stable machine/device IDs and exact addresses/ports. Preserve existing policies.

## Inputs, outputs and authoritative sources

- [deploy/installer/tailnet-plan.example.json](../../deploy/installer/tailnet-plan.example.json)
- [Access request and approval contract](../ACCESS-ENROLLMENT.md)
- [dashboard/admission.py](../../dashboard/admission.py)

The linked code/schemas define fields, enums and state transitions. This document does not duplicate them.

## Call contracts

| Boundary | Input | Result or failure | Consumers |
|---|---|---|---|
| `deploy/tailnet.py#enrollment` | Expected tailnet, hostname/tag/SSH preferences | Ready or bounded login/approval-wait result; never switches an existing tailnet | installation |
| `deploy/tailnet.py#configure` | Instance root, validated plan, MagicDNS name | Backed-up API auth config and Compose overlay; no implicit restart | installation, identity |
| `deploy/tailnet.py#serve` | Instance ports and planned HTTPS routes | Existing nonconflicting routes retained; collisions fail before route mutation | installation |

## Related features and change impact

[identity](identity.md)

When enrollment or proxy behavior changes, verify installation plans, exact trusted peers, callback URLs, reruns/pending approvals, TLS and policy scope.

## Tests and failure boundaries

- [tests/deploy/test_tailnet.py](../../tests/deploy/test_tailnet.py)
- [tests/dashboard/test_admission.py](../../tests/dashboard/test_admission.py)
- [tests/deploy/test_access_setup.py](../../tests/deploy/test_access_setup.py)

Test sources define the concrete scenarios. `deploy/test.sh` records individual results as JSON/JUnit.

## Live validation

Passing mocks does not replace actual administrator login, device approval or policy permissions.

## Funnel gateway

The default receipt-based enrollment runs in `loom-gateway`, with shared request
state but no management credentials or Dashboard session keys. Dashboard retains
its admin routes and reconciliation worker. The gateway reuses entry theme/shape
assets and submits real requests; it never proxies Jobs or MCP. `/gateway/probe`
is served only by the private Dashboard, frames only under the configured public
origin, and reports verified identity using a nonce-bound postMessage. A successful
browser probe triggers a fixed private redirect; receipt approval alone does not.
Probe timeout or browser network blocking leaves the request/retry UI available.

## Machine Sharing

New installations use `enrollment_mode: sharing`. `dashboard/sharing.py` owns durable
email/installation deduplication, single-use device invitations, accepted-identity approval,
scoped grants and revocation. Existing membership records are preserved and not dispatched.
The public gateway has no management credential; only the private worker calls DeviceInvites.
See ACCESS-ENROLLMENT.md and test_sharing.py for state transitions and failure contracts.

Publishing the public gateway also grants tailnet members/shared recipients its
exact TCP port on this machine. MagicDNS can select the VPN address after sharing,
so Funnel publication alone is insufficient. This exception never includes the
Dashboard/MCP ports; the pre-approval guard permits only that dedicated gateway
port. Publication validates the listener and uses ETag-protected policy updates.
Every gateway state shares the Dashboard wordmark and a centered hostname directly
below it. State content is limited to email, short status and the applicable request
action; a shared Retry connection control sits outside the card. Approval advances
automatically after a verified private probe, without a separate completion page.
The gateway refreshes on focus/visibility return and displays a polling failure
instead of silently retaining a wait state. The retired `/auth-demo/` entry returns 404; the current Gateway redirects
to the public gateway when a separate gateway is configured.

The gateway combines request entry and email input. Invitation sender metadata is
resolved privately from the invitation sharer ID, persisted with the request, and
exposed only through that receipt's request response (or the admin view). Missing
sender metadata does not block enrollment. Invited requests and preparation errors offer Use another email to the original requester;
a request older than 60 seconds without a resolved invitation shows a delay, not
a fabricated account rejection. Provider errors never render raw text in the UI.

The gateway remembers only the last successfully submitted email in origin-local
browser storage for input convenience. It never treats it as identity, restores
access from it, or auto-submits it; blocked storage leaves entry functional. Use another email explicitly clears the input. Approval waiting uses a static status (no busy
spinner) and explains that the visitor may close the page and return later.

Request rows expose `can_cancel` and audit-derived `user_cancelled`. Public reset
checks cancellation eligibility in the same transaction as withdrawal, so an
approval racing the button cannot be revoked by a receipt. History retains the
original request, cancellation actor/time and normal cleanup state. A different
receipt with the same email cannot cancel it. No email/account-existence lookup is
claimed by the invitation-success UI. The host card starts hidden until session
metadata supplies a hostname; loading/failure remains outside the card with retry.

Gateway materials use translucent backgrounds on cards, inputs, primary controls
and theme controls; text opacity stays intact. Motion tokens share a cubic-bezier
ease: 440 ms card entrance, 420 ms height morph, 260 ms content crossfade without a blank
stagger. Existing brand/host elements remain mounted. Unchanged polling does
not replay motion; rapid state changes cancel superseded animations/ghosts, and
prefers-reduced-motion skips transitions. Presentation never delays API state or
approval/cancellation. Reference: https://toss.tech/article/interaction (shared
easing/timeline principles; the durations here are Loom-specific).

Outgoing content snapshots freeze computed layout/type styles before removing IDs;
otherwise ID-scoped icon dimensions and text styles would change during fade-out.
Card entrance runs independently of content retargeting. Interrupted transitions
start from the rendered height/opacity; visual-content changes also retarget the
morph, while unchanged polling remains still. Geometry uses layout dimensions so
entrance scaling does not distort the squircle or the next height target.

The generated Docker network keeps dynamic addresses in the upper half of the configured /24. Ops and Dashboard use fixed addresses outside that pool, so a restarting Gateway cannot claim the Dashboard ingress address. Existing user network overrides remain authoritative.
