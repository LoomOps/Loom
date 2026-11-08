# 7. Final verification and user handoff

Run the packaged CPU smoke Job as part of the authorized installation, then rerun the complete doctor:

```bash
sudo python3 /absolute/kit/setup.py smoke /absolute/install-plan.json
sudo python3 /absolute/kit/setup.py doctor /absolute/install-plan.json
sudo python3 /absolute/kit/setup.py handoff /absolute/install-plan.json
```

If any required check fails, report installation incomplete, its failed stage and next action. Never describe a reduced installation as complete or hand out the owner token as a workaround. Do not treat the install progress file as a current health report.

The final user message must contain:

1. **Gateway URL first** — the normal starting point, not the token screen or direct backend.
2. Installed release/build and hostname; service-ready versus remaining live validation.
3. Private Dashboard and MCP URLs, clearly marked as Tailscale-only.
4. Confirmed Admin identity and native Tailscale/Sharing behavior; never include tokens.
5. Running Ops/Dashboard/Gateway and host executor; CPU/GPU selection and exact verification scope.
6. Default output path and every registered RW store with host-to-container mappings, plus dependency path.
7. Doctor and smoke results, report locations, and any untested external-client/reboot/GPU-compute cases.
8. Update checks are enabled/configured or explicitly unavailable; updates do not install automatically. Point to loom_use and the update guide.
9. Offer to install/link `loom_use` (skill name `loom-use`) and automatically register the verified private `/mcp` endpoint in the user's agent client. Ask which client/machine and user profile to configure; do not assume the server is the client or use the `loom` service account's home.

Do not add unrelated platform explanations. Be concise, but never hide an unfinished required stage. Reread INSTALL.md and this checklist once more before sending the handoff.

## Optional client setup after installation

Offer this explicitly in the final handoff: “Would you like me to install loom_use and register this Loom MCP connection in your agent client?” If that setup is already authorized and the target is known, carry it out without asking again.

- On this host, use `/loom/loom_use/current/skills/loom-use` for a supported user-scope skill link. On a remote client, obtain the matching authenticated `/api/guide/bundle`; do not assume the server's filesystem paths exist there.
- Register one **Streamable HTTP** MCP server at the verified private Dashboard origin plus `/mcp`. All six tool groups come from that server. Use the client's supported registration tool/configuration and preserve unrelated connections; detect an existing matching entry instead of duplicating it. Prefer a machine-qualified connection name when the client uses more than one Loom installation.
- Use the user's approved Tailscale identity. Do not copy the installer/owner token, register the public Funnel URL as MCP, or manufacture identity headers. Use the provided stdio adapter only when the client cannot use HTTP MCP directly.
- Use only a client/machine that is available and authorized. If the requested remote client is unavailable, provide its endpoint, bundle location and client-specific next steps; report registration as pending. Do not access another machine solely to complete this optional step.
- Verify `initialize`, `tools/list`, `system.status` and `system.guide` from the actual configured client where possible. Check the machine and returned user permissions. Report skill setup and MCP registration separately, with verified/pending status; server health alone does not prove client registration.

Declining or deferring client setup does not make an otherwise verified server installation incomplete. It remains a suggested client-side follow-up, not another server installation gate.
