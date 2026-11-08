# Tailscale authentication and installation

## Current authentication boundary

- Private Dashboard and MCP sit behind Tailscale Serve. Trust user headers only from exact server-configured proxy peers.
- Dashboard matches the current VPN identity against explicit internal users in `network_users` or approved machine shares, then issues a local session. A cookie cannot impersonate another VPN account.
- Admin privileges require an exact configured `(issuer, subject)` match. Tailnet membership or an email request alone does not grant Admin.
- Every request rechecks current network identity and Sharing approval. Sessions have a fixed default lifetime of seven days, not extended by activity or restarts. Theme cookies are independent of authentication.
- MCP uses trusted Serve identity or an explicit API credential. Gateway request receipts are not authentication credentials.
- Authentication is local to the trusted Tailscale ingress. Unsupported authentication routes return 404.

`issuer` is a persistent local identity namespace, never a fetched endpoint. Keep its exact value and subject mappings across upgrades so existing Owner/Admin references remain valid.

## New installation

Complete `deploy/installer/tailnet-plan.example.json`. In `network_users`, explicitly list internal users allowed on this machine with stable subjects. External shared users enter through Sharing approval. Match Admin issuer/subject in `access-plan.example.json`. The default new issuer is `<Dashboard origin>/identity`.

1. `sudo python3 deploy/tailnet.py enroll --plan /absolute/tailnet-plan.json`
2. The administrator completes the displayed Tailscale login/device approval steps.
3. `sudo python3 deploy/tailnet.py configure --root /loom/ops --plan /absolute/tailnet-plan.json`
4. After applying Compose, use `serve` to connect the approved private origin.
5. Gateway setup is mandatory for a complete installation: follow the numbered installer steps and the [Sharing contract](ACCESS-ENROLLMENT.md). Only the separate request listener is public; Dashboard/MCP remain private. The installer grants explicitly configured native users the exact private machine port.

Management API tokens and machine login are independent. Store secrets only in root-managed files. tag:loom supports Hub discovery; user grants target exact addresses/ports of the approved machine.

## Development identity display

A no-auth dev Dashboard may display the current Serve-provided login and name after
its upstream identity check, but does not infer Admin permission. Set
`LOOM_MANAGEMENT_ORIGIN=https://<machine>:<private-port>` in the development instance
environment to offer a link to deployed access management. Compose applies this
setting only to the development Dashboard. Keep that port restricted to developers.
