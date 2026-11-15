# Dashboard — Item/index impact and detailed work

Status: proposed. Impact: **Direct**. No runtime change is made by this plan.

## Observed baseline

The parent currently loads /api/dashboard/catalog and relays allowlisted operations to a sandboxed iframe. Exact Scenario record links and native detail adapters already exist.

Reviewed source:

- [dashboard/workspace/parent.js](../../../dashboard/workspace/parent.js)
- [dashboard/workspace/frame.js](../../../dashboard/workspace/frame.js)
- [dashboard/workspace/details.js](../../../dashboard/workspace/details.js)
- [dashboard/workspace/explorer.js](../../../dashboard/workspace/explorer.js)
- [dashboard/workspace/scenarios.js](../../../dashboard/workspace/scenarios.js)
- [dashboard/workspace/build.py](../../../dashboard/workspace/build.py)
- [dashboard/server.py](../../../dashboard/server.py)

## Implementation / consumer review

1. Add a lightweight authorized summary/bootstrap and paginated search route so discovery does not wait for an entire growing catalog.
2. Use a compact search surface with meaningful kind/Scenario/time/source filters; keep current Runtimes navigation and concise labels.
3. Render bounded Item details with native links, exact revision/provenance and related files/logs. Fetch long metadata and log ranges on demand.
4. Add file role/name/path and per-Attempt/stage/effect log navigation. Separate historical/current observations and show lag/gaps only when they affect the result.
5. Extend the parent relay with exact action/path/schema allowlists; keep credentials outside the iframe and preserve sandbox/CSP.
6. Keep hidden/support/test records directly reachable without promoting them into main lists; queue visibility remains authoritative.
7. Rebuild generated HTML and verify real native service/relay/Firefox, long decimal IDs, Unicode, XSS and light/dark 1440/500px.

## Invariants

- No boilerplate paragraphs, empty success on unavailable search or full inventory dumped into the UI.
- No arbitrary iframe API forwarding or raw host path access.
- Search uses common IDs; Job controls still use actual typed native refs.

## Planned acceptance and dependencies

Cases: `UI-01`, `UI-02`, `UI-03`, `SCN-01`, `AUTH-01`, `QUERY-03`, `QUERY-04`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [api](api.md), [catalog](catalog.md), [scenarios](scenarios.md).
