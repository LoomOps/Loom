# Optional stdio adapter

Clients supporting HTTP MCP should connect directly to Loom's `/mcp`. For stdio-only clients, `stdio.py` forwards JSON-RPC to the HTTP endpoint. The adapter does not hard-code tool names or agent roles.

```bash
python3 -B /path/to/loom_use/clients/stdio.py /private/path/loom-client.json
```

Example client configuration:

```json
{
  "endpoint": "https://your-loom-host/mcp",
  "credential": "/private/path/loom-credential.json"
}
```

The credential file contains `{"token":"personal-token"}` and must be owned by the executing user with mode `0600`. Never place actual tokens or machine-specific settings in Git or shared bundles. Use HTTPS, or HTTP on loopback only.

For servers configured with Tailscale user authentication, replace `credential` with `"authentication":"tailscale"`. The adapter's machine must have the required Tailscale connectivity. Serve supplies identity headers; clients must not set them directly. The adapter handles negotiated MCP version headers, JSON and per-request SSE responses.

The adapter only connects to the server. Clients call `initialize` and `tools/list`, then read the current server contract through `system.status` and `system.guide`. Event consumption and agent startup or reactivation remain the caller's responsibility.
