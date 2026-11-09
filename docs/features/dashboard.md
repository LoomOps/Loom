# dashboard — Dashboard HTTP relay·workspace UI

## Responsibilities and boundaries

Never send credentials to the opaque iframe. Browser writes respect Origin/CSRF boundaries. The UI must not infer completed backend states.

## Inputs, outputs and authoritative sources

- [dashboard/server.py](../../dashboard/server.py)
- [dashboard/workspace/parent.js](../../dashboard/workspace/parent.js)

The linked code/schemas define fields, enums and state transitions. This document does not duplicate them.

## Call contracts

| Boundary | Input | Result or failure | Consumers |
|---|---|---|---|
| `dashboard/server.py#WebHandler` | Allowed read route or queue mutation + authenticated browser | Bounded response; session/Origin/CSRF guard for writes; no generic arbitrary proxy | api |
| `dashboard/server.py#make_server` | Ops origin, optional SSO config and exact trusted ingress | Dashboard server with explicit authentication mode | installation |

## Related features and change impact

[identity](identity.md) · [api](api.md) · [catalog](catalog.md) · [observations](observations.md) · [onboarding](onboarding.md)

Rebuild the generated workspace after UI source edits. Verify both HTTP behavior and real Firefox for authentication, session or relay changes.

The login profile sits below sidebar software information and opens the `Tailscale` tab available to all users. Ordinary users see Account. Administrators additionally see Member and this machine's Access Request approval, denial and retry controls. No logout button is displayed. Tailnet roles and Loom administrator privileges are independent; administrator identity uses the existing issuer/subject allowlist.

Keep session/CSRF state in the parent only. Restrict iframe messages to permitted queries and decisions. Member data and administrative APIs also enforce administrator authorization server-side. Never suspend or delete users across the entire tailnet. The network feature defines Sharing acceptance, approval and revocation. Only an open administrator tab refreshes every 15 seconds; report failed queries rather than displaying an apparently empty list.

## Tests and failure boundaries

The Extensions sidebar entry uses a puzzle icon, distinct from the
Troubleshooting wrench. Its shared inventory filters by software type, namespace
and text, separating registered immutable extensions from preserved catalog
records. Details display source pins, revision history, execution stages and exact
Tool/Profile bindings. Old revision URLs remain readable after a newer registration;
per-document revision/filter selection survives catalog refresh. No browser action
registers, executes or rewrites software. Native extension ownership is integrated
in its Identity panel rather than a duplicate generic Properties card.

- [tests/dashboard/test_proxy.py](../../tests/dashboard/test_proxy.py)
- [tests/dashboard/browser/test_workspace.py](../../tests/dashboard/browser/test_workspace.py)

Test sources define the concrete scenarios. `deploy/test.sh` records individual results as JSON/JUnit.

## Live validation

Actual Safari behavior, existing user sessions and trusted Tailscale ingress require separate acceptance testing.

With a separate gateway configured, an unauthenticated private `/` redirects to
that gateway before sending workspace HTML. `/session.entry_url` handles a session
change during page loading without exposing the old token form. The old `/node` and external login callback paths return 404. No browser token-entry fallback remains. An installation missing native identity
or gateway setup stays closed and offers connection retry; administrators must
repair its installation configuration. Agent tokens remain an API/MCP credential,
not a browser sign-in method. The API root
returns service discovery JSON, not a second Dashboard. An initial transport failure
shows a retry action instead of an empty page.

## Shared theme preference

The trusted parent and entry page share the host-only `loom_theme` cookie
(`light`, `dark`, `system`; Path=/, SameSite=Lax, one year, Secure on HTTPS).
Absent or invalid preference defaults to the computer setting. The opaque frame
requests changes through source-checked messages; it receives only the selected
mode. Saved workspace state cannot override this cookie. Visibility/focus refresh
keeps pages in sync; system mode follows OS appearance without storing its resolved
light/dark value. The cookie contains no identity or authentication data.

## Content surfaces

Only `.pa-pane-body` receives the entry page's subtle gradient, surface spacing
and continuous card corners. Sidebar uses a bottom-brightening gradient and a
left-faded selected menu; document tabs keep their existing style.
`workspace/surfaces.js` draws non-interactive SVG backgrounds with n=3 corner curves
for large panels, resource cards, metrics and MCP detail cards. Content is not
clip-pathed, so menus and focus rings can extend outside a card. A shared ResizeObserver updates dimensions before paint and skips unchanged sizes;
disconnected cards are unobserved. Each scrolling pane prepares filtered card skins
within an 800px halo above and below its viewport, releasing the compositor hint
outside that range. This changes rendering preparation, not data fetching. Tables and
badges preserve their own geometry. Colors follow the shared light/dark/system choice.
The pane gradient uses `background-attachment:scroll`, anchored to the viewport
box rather than the scrollable content height; it does not require a scroll to
repaint after initial content sizing.

### Rounded widget inventory

Applied through the common surface renderer to panels, resources/metrics, individual
resource tiles, result flows/highlights, engineering/definition panels, discarded
results, notices/warnings, media/episode previews, pipeline stages, file explorer,
MCP code/detail blocks, readable text/pre blocks, and preserved source/list widgets.
The selector inventory is maintained in `workspace/surfaces.js` (`widgets`).
Nested widgets retain their original semantic background; warnings retain warning
colors. Async replacement of a card's contents reinstalls its missing skin.

Outside this widget change: sidebar/navigation/document tabs; native form controls
and buttons; badges/pills; activity heatmap cells/legends; circular indicators and
progress tracks. These are navigation, controls, or data marks, not panel surfaces.

Telemetry/catalog reconciliation preserves client-owned surface SVGs and their
class, rather than deleting and re-adding them for each sample. The resource
refresh regression test verifies DOM identity and absence of decorative removals
while values change; no polling cadence or data collection contract changes.

Background refresh also reconciles Docker/MCP panels, sync status and sidebar
software information in place. Container rows/details are keyed; expanded rows,
focused controls and MCP selection survive an updated sample. Rendered Lucide
SVGs are preserved when the icon name is unchanged, and the icon scanner skips
existing SVGs. Open media/text previews remain mounted during catalog refresh.
Browser regressions cover changed telemetry/catalog revisions, Docker values and
MCP refresh, checking both new content and retained DOM/interaction state.

## Explorer storage scope

Explorer favorites list only connected `data/<id>` mounts, without a duplicate
Data Storage entry. Explorer shows the catalog's registered files under those roots,
using storage observations when available and indexed data roots before observations
arrive. It omits service directories and unmaterialized historical `references`.
This is a presentation filter, not an authorization boundary or a filesystem scan;
record details retain their original provenance. Empty observed storage roots remain
visible. The latest two folder columns are shown; the complete clickable breadcrumb
allows returning to older ancestors. Folder columns have a fixed 300px width; they
do not divide the available width. The right inspector fills the remaining space
(minimum 300px), containing the selected file preview above its metadata. Narrow
views scroll horizontally inside Finder, without overflowing the page. File content
still loads only when the user requests its preview.

Tailscale navigation uses the official Tailscale press-kit symbol (https://tailscale.com/press),
inlined in workspace/icons.js with theme-inherited color. In Sharing mode Member lists
the authenticated Admin pinned first, followed only by this machine's granted
share recipients (ready, approved, and matching the current accepted identity). The administrator row is not a share and has no Revoke action; Tailnet
membership is not queried. Observed acceptance time orders managed shares, creation time orders external
shares. Member uses a Role column with Admin/Member; Access Request uses English
Grant status labels. Pending requests and unapproved external shares are excluded
from Member. Account, Member and
Access Request use scoped responsive styles; narrow panes stack labeled table cells without
page-level overflow. Browser coverage includes long emails, owner ordering and both themes.

## Workspace reload

The parent stores the current workspace in this browser tab's history entry:
open tabs, their order, selected tab and split-pane layout survive reload. New
browser tabs do not inherit it. Restoration validates the workspace and matching
URL before the frame's initial state can overwrite it. Catalog data and credentials
are not stored with the workspace; appearance retains its separate theme cookie.
Closed documents stay closed after reload. Access Request offers Admin Remove for
completed denied/revoked records, preserving audit entries and rejecting removal
while revocation is still in progress.

The Loom Ops wordmark keeps Loom in the foreground text color and colors only Ops
with a theme accent. The sidebar uses the motion sample palette through the shared
`--loom-brand-accent` token: light #436fc7, dark #9bb9f1. Ops, active navigation,
icons and focus accents share it; selection/tint backgrounds derive from it without
changing semantic status colors or the content-area palette.
Logo motion studies remain review artifacts until a concept is selected.

## Activity scrolling

`workspace/activity.js` binds each calendar once. Visible geometry from ResizeObserver
initializes today's position; a hidden frame never saves a zero-width scroll position.
Native wheel/touch scrolling is preserved. Range expansion waits for scrolling to
settle and replaces only the calendar track, retaining its date anchor and the
scrolling element. Catalog reconciliation waits for calendar/list scrolling to idle
as well as the main pane. Browser coverage includes hidden-to-visible initialization,
range expansion, repeated initialization and the Today control.
Year labels sit above the month row, follow the January column, and clamp to the
visible calendar edges. Year segments travel natively in the same scrolling track as January; the current
year uses a separate sticky marker. Scroll events only switch year visibility,
never write moving label positions. When labels come within 8px, the new year immediately replaces the
previous one at the edge;
reverse scrolling restores it. Labels are repositioned after range expansion,
resize and catalog reconciliation without replacing the scrolling element.

Catalog reconciliation mutates its already detached candidate nodes when retaining
client-owned card skins, avoiding an additional deep copy of every panel subtree.
DOM identity, expansion state and media preservation remain covered by browser tests.

## Brand motion

`dashboard/branding` owns the Execution Slot geometry and decorative motion.
The Dashboard embeds the SVG/CSS with a 12-second cycle and longer empty/closed
intervals; it never represents actual queue activity. The entry page uses the
8-second cycle and lazily loads the Canvas background after UI paint. Both reuse
the SVG paths. Canvas caps drawing at 30 fps, stops while hidden and honors
reduced-motion; the flat mark uses CSS transforms/opacity. Theme changes redraw
the background without reloading it. Authentication and request stages do not
depend on either animation loading. The acrylic card filters only its backdrop (30px blur); the scene itself keeps
its independent 19px blur. Card tint is 34% in either theme, with 60px n=4
corners and no outline or inset highlight. Verify acrylic samples from the rendered
display: Firefox WebDriver screenshots omit backdrop-filter effects.

## Public template data

`workspace/template.html` is now a sanitized, hash-pinned UI template. Its catalog
and metric snapshots are synthetic fixtures, not preserved research records or
operational agent instructions. Production generation removes the fixture catalog
and loads live data only through the existing authenticated API. A privacy check in
`tests/deploy/test_public_source.py` prevents embedded real catalog records and verifies
the generated workspace matches the sanitized template. Display normalization logic
is preserved independently of the example data.

## Resident token presentation

The Admin token card lists active credentials first and keeps revoked records in a
collapsed history section. Revocation is not deletion. The issuance dialog lives in
the parent document, uses the workspace typography/theme, and clears the secret on
close. Secrets never enter the catalog iframe or browser storage. Browser tests use fake credentials. The issuance dialog only displays/copies the token; installation commands belong to the client setup flow. Revocation confirmation runs in a themed parent dialog, never native `confirm()` inside the sandboxed iframe. Cancel/Escape makes no request; confirmation uses the existing Admin/CSRF API.

Development sessions display a trusted Serve identity when present, without granting
Admin or enabling a production admission worker. An optional `LOOM_MANAGEMENT_ORIGIN`
HTTPS origin on the dev Dashboard links to the separately deployed administration UI.
Anonymous dev access is labeled as development authentication bypass, not a failed
Tailscale login. This display metadata does not change API authorization.

The MCP reference tab and page are displayed as **Loom Use**. The route `mcp`,
tool groups and protocol names remain stable; the server and guide manifest expose
`title: "Loom Use"` for display. Package paths and the `loom-use` skill identifier
remain unchanged.
