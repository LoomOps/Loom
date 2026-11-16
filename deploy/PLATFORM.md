# Platform architecture

Loom owns scheduling, execution and resource-return evidence. Producers own algorithms, data semantics and output manifests. External agents own user interaction and repair decisions. No particular agent hierarchy or resident supervisor is required.

## Boundaries

The native Ops service combines a transactional writer, read projections and independent background collection. `loom-host` controls actual processes/containers and resource ownership. A successful RPC, process exit, artifact publication and event acknowledgment are different facts. Unknown executor return fences dispatch until reconciliation resolves ownership.

Dashboard retains the established web implementation and identity integrations. It has no Ops DB or Docker socket mount. The parent mediates authenticated API calls; the workspace iframe receives scoped data, not credentials. Private Tailscale routes carry Dashboard/MCP; the separate public Gateway handles access requests and redirect only.

See [implementation ownership](../docs/IMPLEMENTATION.md) for module entry points, [concepts](../docs/CONCEPTS.md) for the object model, and [API reference](../docs/reference/README.md) for exact caller contracts. These are the current reference; this page does not duplicate action lists or lifecycle enums.

## Persistence and environments

Accepted Job requests, execution revisions and provenance remain immutable. Current responsibility, presentation and verified copy availability are additive observations. Common Items connect records for discovery; file manifests and a separate disposable index avoid putting every payload file in the execution DB.

Profiles bind exact Component/Tool and Runtime revisions. Runtime metadata lives in the host catalog; immutable images, configured dependency roots and validation evidence must be preserved together. Neither registration nor an installed package list proves scientific/GPU acceptance. External Python SDKs can remain pinned worker integrations while Rust owns producer control and platform state.

## Deployment

New machines follow [INSTALL.md](../INSTALL.md). Existing instances follow [UPDATES.md](../docs/UPDATES.md); cumulative migrations preserve installation identity, credentials, registered software, histories and external storage references. Drain actual execution and callback owners before switching code. Never run the old writer against a migrated native DB.

Development uses separate instances, data, ports and credentials. Numbered release images are immutable. Authentication bypass is development-only. Deployment checks and feature verification are not additional user Job gates.
