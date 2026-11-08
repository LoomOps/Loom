# MCP client setup

## Resident Codex

Ask a Loom Dashboard Admin to issue a token in **Tailscale → Agent Tokens** for your registered Loom user. It has no expiration; Admin can revoke it. Run this on the machine/account running Codex, using the downloaded Loom Use bundle path (or `/loom/loom_use/current` on the server):

```bash
python3 /path/to/loom_use/clients/setup.py configure \
  --endpoint https://your-loom-host/mcp --name loom-use
python3 /path/to/loom_use/clients/setup.py doctor --name loom-use
```

Paste the token only at the hidden prompt, never in the command or chat. Default storage: `~/.codex/loom/loom-use/credential.json` (or the configured CODEX_HOME). The directory is 0700 and files are 0600. Use `--directory /absolute/project/.loom-credentials` on both commands to select a project-local private directory; the script adds an ignore-all `.gitignore`. Do not use a tracked credential path.

Setup checks initialize, tools/list, system.status and system.guide before saving the token. It preserves unrelated Codex settings, backs up existing config privately, and registers one native Streamable HTTP server using a local `http_headers_helper`. Reconnect that MCP server in Codex after setup. The helper only reads the file: no sudo, network calls, issuance or renewal. Doctor verifies configuration and live authentication; it does not start a Codex session or submit a Job. An unavailable token leaves client authentication pending without invalidating a completed server installation.

Token authentication does not grant network access. Keep using the verified private MCP endpoint. For personal Tailscale devices with a verified user identity, the existing tokenless route remains available. The helper is supported for Codex HTTP connections made in its local execution environment; run it on the resident Codex host, not on an unrelated laptop or remote executor.

[Codex HTTP helper reference](https://learn.chatgpt.com/docs/extend/mcp).

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

The default MCP registration name is `loom-use` for both `configure` and `doctor`.
You may omit `--name`. Use an explicit alternate `--name` only when requested or
when connecting another server under a distinct name. An existing name bound to
a different endpoint is never overwritten automatically.
