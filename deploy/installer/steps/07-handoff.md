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
8. Update checks are enabled/configured or explicitly unavailable; updates do not install automatically. Point to Loom Use and the update guide.
9. Offer: "Install Loom Use in this Codex and connect to this Loom?" Default to the
   current agent machine and current OS user. Ask only for a different target if the
   user requests one. The verified private endpoint is already known.

Reread INSTALL.md and this checklist before the final handoff.

## Optional independent client handoff

After authorization, obtain the exact Loom Use release in client_setup.release.
Use `python3 /path/to/platform/deploy/client_release.py --destination /user/source/path`
to download/check the pinned source ZIP, or clone https://github.com/LoomOps/Loom-Use
and checkout its specified tag. The returned source directory has INSTALL.md.
Read that INSTALL.md and hand it the verified private MCP address from connect_argv.
Do not duplicate its installation, token, registration or reconnect logic here.

The target is the current user's Codex, not the Loom service account. Do not access
another machine unless selected and authorized. The independent installer detects
client Tailscale identity and adopts valid exact-endpoint credentials. Resident or
tagged Codex uses a token issued by Dashboard > Tailscale > Agent Tokens. Token
entry is hidden; no automatic issuance, sudo renewal or owner credential reuse.

Client setup requests actual app refresh and verifies active-thread tool calls where
the client control API is available. Report configuration/transport/runtime separately.
If the user declines, lacks a token, or the app requires manual reconnection, retain
server complete / client pending. A client limitation does not invalidate a verified
server installation. Existing server-side Skill bridges remain readable; new user
Skill links are managed exclusively by the independent client.

Offer the optional Loom SSH Skill from the same pinned Loom Use release. Same-host
agents use local tools and need no account issuance. For remote analysis, reuse
existing authorized SSH or direct the user to Admin → Tailscale → SSH. Never use
an MCP token as an SSH password. Report managed SSH readiness separately from
external login verification.
