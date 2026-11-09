# ssh — Personal host access

Loom Admin manages optional personal host accounts; the loom service account stays
noninteractive. dashboard/ssh_access.py owns durable enable/disable intent and exact
Tailnet rule changes through the existing ownership journal. deploy/ssh_host.py is a
root Unix service with fixed operations and a kernel peer-UID check. Only the private
Dashboard receives its socket; Ops receives readonly per-principal metadata. The
public Gateway receives neither. See [SSH](../SSH.md).

The host registry binds owner, account name, UID, home and GECOS marker. Existing
accounts are never silently adopted. Personal accounts use the loomu_ namespace,
locked passwords and no sudo/docker membership. Enable stages account creation,
policy validation/write and shell enablement; failure is explicit and retryable.
Disable locks the shell/expiry and terminates that UID before policy removal. Homes
remain. Shared-access revocation calls the same cleanup before completing. Jobs use
the executor identity, not the personal SSH UID, and are not cancelled by SSH revoke.

No host account/SSH setup is required for same-machine clients. system.ssh returns
only the authenticated principal's connection and nonsecret machine addresses. The
optional Loom SSH Skill verifies locality, then reuses authorized SSH or requests
Admin enablement. It never uses MCP tokens as SSH passwords or bypasses host keys.

Installation uses host_ssh in the plan (new plans true, old absent plans unchanged).
ssh_setup doctor checks actual service/socket and Tailscale SSH, not an install flag.
Dev-build does not receive a privileged host socket; tests isolate that boundary.
Updates retain host registry/homes and refresh protected helper code. Uninstall
locks managed accounts and removes only journal-owned policy; homes are retained.

Regression tests use real temporary SQLite/files, mocked account commands/provider
API, permission rejection, partial-failure retry, UID replacement and per-principal
metadata. External device login, live key trust and existing-session termination
require separate live evidence. Feature checks are development-only, not Job gates.
