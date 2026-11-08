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
6. Primary output path and every additional read-only store, plus dependency path.
7. Doctor and smoke results, report locations, and any untested external-client/reboot/GPU-compute cases.
8. Update checks are enabled/configured or explicitly unavailable; updates do not install automatically. Point to loom_use and the update guide.

Do not add unrelated platform explanations. Be concise, but never hide an unfinished required stage. Reread INSTALL.md and this checklist once more before sending the handoff.
