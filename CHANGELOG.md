# Changelog

## 0.1.0-alpha.28 — 2026-11-08

- Remove the retired browser token-entry fallback; missing native identity now stays closed with a retry action.

- Align installer defaults and documentation with Gateway 443 / private 8443; keep explicit existing ports.
- Remove obsolete local short-lived credential issuance; preserve existing expiry semantics.
- Support verified Tailscale guide refresh with principal-scoped caches and consistent private credential reads.
- Strip all client token rows on export and restore; reject schema-1 exports that would omit secondary-store payloads.
- Review all tracked files, feature verification boundaries, dev/alpha packaging and current instructions.

## 0.1.0-alpha.27 — 2026-11-08

- Fix sandbox-blocked token revocation with a parent-owned confirmation dialog.
- Remove duplicate connection instructions from token issuance; installer supplies exact private-endpoint setup commands.

## 0.1.0-alpha.26 — 2026-11-08

- Align Agent Token forms/dialogs with the Dashboard theme and collapse revoked history.
- Show verified development network identity without implying Admin rights.

## 0.1.0-alpha.25 — 2026-11-08

- Verify deployed native identity, private relay routes and separate public Gateway during update acceptance.
- Repair Admin, update-check and event relay routing.

## 0.1.0-alpha.24 — 2026-11-08

- Add Dashboard Admin-issued, individually revocable agent tokens without expiration.
  Resident clients retain their selected user identity and operational scopes.
- Add private token storage, Codex Streamable HTTP setup and read-only client doctor.
  Installation offers client setup without automatically issuing credentials.
- API revision: `loom-ops/v38`; six MCP groups and Job contracts remain unchanged.

## 0.1.0-alpha.23 — 2026-11-08

- Guide existing Tailscale connection reuse, initial login, explicit account switching
  and device approval separately from the Loom Admin email and management credential.
- Journal installation and runtime policy/tag ownership, reconcile uncertain writes,
  and restore only owned changes during uninstall. Preserve reused manual invitations
  and shared resources; detect final-device-tag reauthentication before stopping services.
- Add reviewed, policy-only cleanup for removed installations, plus installation
  doctor, offline-kit dependency and regression coverage.
- Include the ownership journal module in the explicit Dashboard build context.
  Alpha.22 failed packaging and was never published or deployed.

## 0.1.0-alpha.21 — 2026-11-08

- Add an administrator-only uninstall CLI that previews and removes an instance's
  managed Sharing grants/invitations, listeners and local access history. Remote
  cleanup is verified before deleting credentials; interrupted cleanup can resume.
  Jobs, artifacts, shared host dependencies and the host's Tailscale identity remain.

## 0.1.0-alpha.20 — 2026-11-08

- Expose artifact location repair as `results.reconnect` through MCP, generated
  reference/bundle and usage guides, matching the existing HTTP operation.
- Add HTTP/MCP parity and tokenless Tailscale MCP regression coverage; document
  instance-specific authentication and Sharing port reconciliation on upgrades.
- API revision: `loom-ops/v37`. Existing credentials, Job pins and data are preserved.

## 0.1.0-alpha.19 — 2026-11-08

- Default new installs to host /loom/data as data/0; prompt for optional RW stores and existing filesystem mounts. Preserve existing data owners and check effective service access.
- Add jobs.submit.output_storage, persistent storage identities, selected Attempt workspaces and result DB locations. Secondary outputs support catalog previews, result reconnects and hash-checked downstream input mounts. Existing Jobs retain their primary output storage.
- Expose store discovery and output selection through the same HTTP/MCP contracts; retain six tool groups. Show Output Storage in native Job properties.
- Offer loom_use skill installation and user-scoped MCP registration after verified server installation. Client verification is separate from service readiness.
- Extend storage, executor, result, installer and client-handoff regressions; isolate release tests from installed host systemd units.

## 0.1.0-alpha.18 — 2026-11-08

- Pin the Docker bridge gateway to the exact trusted ingress address. Installation doctor compares live container gateways and trusted-proxy configuration, catching healthy services that cannot identify Tailscale users.
- Distinguish a failed private identity probe from a new visitor. Show connection/administrator guidance instead of prompting an existing user to request another invitation.
- Block duplicate Sharing invitations for native Admins/users, approved recipients and existing tailnet members. Membership lookup failures cannot fall through to sending an invitation.
- Ask explicitly for the Tailscale login email of the account that will administer Loom; update installation steps, diagnostics and operator guidance together.
- Refine Gateway acrylic: 34% tint in both themes, 30px card-only backdrop blur, 60px continuous corners and no white edge highlight. Preserve the scene blur and readable controls.
- Recheck all 15 feature boundaries, regression suites and matching installation/usage packaging. Existing installations retain local configuration and need an explicit idle repair if their bridge gateway differs.

## 0.1.0-alpha.17 — 2026-11-08

- Require the complete native Tailscale/Sharing installation: Ops, Dashboard, Gateway, executor and explicit Admin identity. Reject boolean Tailscale/token-only installation plans.
- Split installation guidance into seven steps, with host-based recommendations, per-step rereading and Gateway-first handoff.
- Add read-only stage diagnostics (`doctor`/`verify`) for actual identities, images, mounts, services, observations and public/private ingress. Progress receipts alone never establish readiness.
- Ship a repeatable CPU preparation/execution/result smoke Job and require its verified artifact before a successful handoff.
- Generate machine-bound plans, support additional read-only catalog storage, and grant only explicit native users the private machine port.
- Prepare the Python execution image explicitly and support packaging recovery from a verified immutable build. Source and offline kits include diagnostics and step documents.
- Review all 15 feature boundaries and extend installer, policy and result-contract regression coverage. Existing installations require scoped configuration reconciliation; this release does not automatically update services or rerun fresh installation.

## 0.1.0-alpha.16 — 2026-11-08

- Establish the public GPL-3.0-only source baseline, with license files in source, images and usage bundles.
- Keep one native Tailscale identity and machine Sharing flow. Remove the unused tailnet membership/device approval backend and obsolete login/admin UI.
- Remove personal operator references, one-off research migration/repair scripts, the old Agent application and pre-component producer execution.
- Expose only the six documented MCP groups. Preserve existing database identities, ownership, history and immutable artifacts.
- Keep GitHub Release discovery, explicit idle updates, local overrides and versioned usage guides; no automatic deployment is introduced.
- Extend regression coverage for unsupported routes, preserved historical data, distribution licensing and all-source syntax validation.
