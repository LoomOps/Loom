# Changelog

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
