# 6. Verify ingress and Sharing

```bash
sudo python3 /absolute/kit/setup.py doctor /absolute/install-plan.json --stage network
```

Doctor verifies exact private Serve routes, the separate public Funnel listener, Gateway HTTPS, exact-machine Gateway grant and absence of operational API/MCP/admin routes on the public listener. Sharing management read access and the pre-approval policy guard must pass. Never publish Dashboard/MCP through Funnel to fix connectivity.

If only publication failed after services/configuration passed:

```bash
sudo python3 /absolute/kit/setup.py resume-network /absolute/install-plan.json
```

On another authorized Tailscale client, open the Gateway first. Verify the current user's profile and Admin/Member role after private redirect. For a real external shared user, verify request → invitation acceptance → Admin approval → private access; do not send invitations to arbitrary sample emails. Use isolated mocks for denied/revoked/unapproved flows until a real test participant is available. Check VPN-account switching cannot reuse another identity's session.

Exit: network doctor passes. External-client checks remain explicitly pending until actually tested. Reread INSTALL.md and the step index before step 7.
