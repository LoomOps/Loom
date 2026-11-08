# Uninstall Loom and clean up machine access

This is an administrator CLI workflow, not a Dashboard action. Implementing or
installing this command does not authorize running it. Ask the operator to review
the plan before uninstalling their selected instance. Do not run it as an upgrade
step or before switching a user's client connection.

Use the source matching the installed release (source/offline kits include it
under `platform/deploy` or `source/deploy`). Keep administrator Tailscale API access
available until remote cleanup has completed.

```bash
sudo python3 deploy/uninstall.py plan \
  --instance /loom/ops --project loom --output /tmp/loom-uninstall-plan.json

# Review instance, machine/device identity, invitations, listeners and preserved items.
sudo python3 deploy/uninstall.py apply \
  --plan /tmp/loom-uninstall-plan.json \
  --confirm-machine MACHINE_ID_FROM_PLAN \
  --receipt /loom/uninstall-receipt.json
```

The plan performs read-only API/state inspection and writes a private plan file.
The apply command requires the installation machine ID from that plan, fences an
idle queue, verifies returned executor resources, and disables/stops the matching
Compose startup unit and services. It never kills active Jobs to uninstall.

Cleanup removes:

- The installation-specific Sharing approval group and its exact machine grants.
- The exact public Gateway access exception created for that listener.
- Outstanding and accepted device invitations tracked by this Loom installation.
- Its private Serve/public Funnel listeners, only if every route belongs to it.
- Its Compose containers/network, without deleting volumes or images.
- Local access requests, receipts, approval audit and policy-backup DB, Dashboard
  sessions, SSO/admission configuration and credentials, and instance network
  configuration backups. Ops Tailscale/Dashboard auth bindings are cleared.

Cleanup preserves the host's Tailscale login/device, tags, native Admin policy,
unrelated Tailnet users/policies/shares/listeners, Job DB/history and owner
provenance, datasets/models/artifacts, storage mounts, components/profiles, service
account, images, dependencies and shared host helpers. It does not delete a Tailnet
user or run `tailscale logout`, `tailscale down` or global Serve/Funnel `reset`.
Those services may be shared with another installation. No global `/loom` deletion
or Docker prune is performed. Retained runtime data can be migrated/restored; a
fresh installer must not be run over it blindly.

Untracked manual shares are shown as preserved. An invitation whose delivery is
uncertain must have its provider ID reconciled before uninstall; the command must
not lose the evidence needed to revoke it. An extended policy group, another
machine in the same request DB, mixed listener routes or approval ports shared by
multiple instances with different service routes requires explicit reconciliation instead of deleting another
service's access. This includes installations whose dev and alpha share one
admission worker. Plan and uninstall those boundaries deliberately.

If a remote operation fails, the receipt says `incomplete`. The services stay
stopped/fenced and management credentials/access records remain until policy,
invitation and listener removal are confirmed. Re-run the same apply command to
resume; it rechecks actual remote state. No new invitations are sent. A completed
receipt is a no-op on repeated apply. A sanitized receipt retains installation
identity/stages, not email lists, policy contents or secrets.

External backups, update archives, Tailscale's own audit history and client-side
cookies/cache/MCP configuration are outside this cleanup. Remove retired client
connections separately. A stale browser receipt is invalid after local cleanup;
visual preferences on the client may remain. This is logical deletion, not a claim
of secure erasure from storage media or provider backups.

The scoped listener commands follow the official
[Tailscale Serve](https://tailscale.com/docs/reference/tailscale-cli/serve) and
[Funnel](https://tailscale.com/docs/reference/tailscale-cli/funnel) `off` interfaces.
Verify remote deletion using a disposable installation before relying on it for a
production teardown. Unit tests mock Tailscale/Docker/systemd; they never uninstall
the development host.
