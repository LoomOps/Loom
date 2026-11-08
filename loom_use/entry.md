# Loom server entry — hand this to your agent

Use the **Loom URL and authentication method** supplied by the administrator. A server configured for Tailscale user authentication allows tokenless access over the authorized tailnet connection. Otherwise, obtain a personal token separately; never include it in shared documents, prompts or repositories. `<Loom URL>/mcp` uses Streamable HTTP. For tokenless access, the client must use an approved user-owned Tailscale device and the private HTTPS endpoint; do not send an old Authorization token, since an explicit invalid token is rejected rather than falling back to network identity. Tagged devices and requests without a user identity need an explicitly issued service credential. Normal usage requires neither SSH nor server sudo access. Use a separately granted SSH account only for remote development or file administration.

1. Call the `system` tool's `status` action to verify the machine, deployed version and permissions.
2. Call `system.guide` with empty arguments to read the current guide index. Start with `entry` and `quickstart`; use `jobs` and `components` for implementation and `recovery` for errors. Example: `{"name":"system","arguments":{"action":"guide","arguments":{"id":"quickstart"}}}`.
3. Read `system.describe({group,action})` for the exact action input. One MCP endpoint exposes six groups: jobs/recovery/events/components/results/system.
4. On a new session or reconnect, compare `system.status.onboarding` with the previous value. Recheck when the short version footer, `_meta.loom` or HTTP `X-Loom-Revision` changes. If only guidance changed, refresh the affected documents; if contracts changed, refresh tool discovery and the relevant action schemas. Do not resubmit existing Jobs or automatically change execution revisions.

Work within the user's authorized scope. Normal execution does not require another agent, manager or resident supervisor. Use the Loom API as the authority for execution, queue and result state; do not edit its database directly. Caller-side operations determine which agent owns recovery and receives events.

For direct host access: `/loom` is service-owned operational state; `/dependencies` contains shared external source and patched dependency recipes. User workspaces may live in the user's home or another chosen location. Administrators operate Compose and the executor. Host filesystem access and Job mount permissions are separate.

If an older server lacks `system.guide`, compare this bundle's `manifest.json` with the actual `system.status` version, then use the available `system.describe` and bundled documentation. Do not assume new features have been deployed. This guide is not an installer that overrides personal settings.
