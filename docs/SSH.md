# Personal SSH access

In Dashboard → Tailscale → SSH, an Admin selects an approved Member and a personal
`loomu_…` account, then enables access. The service creates a private home with
`work`, `components` and `profiles` draft directories. It does not reuse or modify
an existing Unix account. Existing manually configured SSH remains usable independently.
No sudo, Docker group or service-credential permissions are granted.

Authentication uses Tailscale SSH from the person's user-owned Tailscale client.
The exact user, installation and Unix account are allowed. A tagged client is not a
personal identity. Same-machine agents use current local filesystem permissions,
with their existing MCP token; no SSH setup is necessary. No new key-issuance flow
is included for unrelated remote tagged machines.

Host file permissions still apply. Draft Component/Tool/Profile sources are registered using
MCP, which publishes immutable runtime revisions. Do not edit those revisions or
the platform database directly. When installing this capability, only the explicitly registered data stores receive
a dedicated shared RW ACL, including existing files and directory defaults. Ownership
is preserved and symlinks are not followed. Service state and dependencies are not
granted. Each managed account joins only this nonprivileged data group. New data
stores require rerunning SSH setup; doctor reports stale storage bindings.

Disable SSH blocks new logins and terminates processes of this dedicated personal
UID; it retains the home and does not cancel queued/running Loom Jobs. Partial remote
policy cleanup is an error with retry; it is not displayed as successful revocation.
Sharing revocation also disables managed SSH. Account deletion is not automatic.

## Installation and maintenance

New installation plans enable `host_ssh`; set it to false to omit this capability.
For an existing production instance, run as the host administrator:

```sh
sudo python3 deploy/ssh_setup.py install /loom/ops
sudo python3 deploy/ssh_setup.py doctor /loom/ops
```

Apply the instance Compose overlay at an idle maintenance point. The helper code
is root-owned under /usr/local/lib/loom-ssh, registry under /var/lib/loom-ssh, and
socket under /run/loom-ssh. Only the Dashboard mounts the socket. No helper is added
to an isolated dev-build. The protected host registry and homes survive updates.
Uninstall disables owned accounts and removes owned policy while retaining files
and the existing Tailscale login. Do not delete registry records to bypass recovery.

## API

Admin session + CSRF: GET /admission/ssh lists state; POST accepts exactly
`{login, username, action}` with action enable/disable. Login must be an approved
member or the current authenticated Admin. Ordinary callers cannot mutate accounts.
GET /api/ssh and MCP system.ssh expose only the caller's descriptor. A descriptor is
not a credential and never proves live SSH connectivity. Loom SSH doctor checks that.

Helper restarts preserve the socket directory inode. The stack startup unit depends
on the helper so the mount exists before Compose starts. An existing installation
receives the same unit repair during the normal idle update. When adding data stores,
rerun SSH setup and verify doctor; existing Dashboard mounts remain connected.
