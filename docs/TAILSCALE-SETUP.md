# Select the machine's Tailscale connection

Collect three separate choices in the initial installation conversation:

1. The **Tailscale login email of the account that will administer Loom**.
2. The machine's intended Tailnet and whether to retain its current connection.
3. A protected Tailscale management API credential for that Tailnet. Logging the
   machine in does not issue this credential or grant the Loom Admin Tailnet Admin
   privileges. The installer verifies provider access separately.

Run from source (in a kit, replace the script path with `setup.py`):

```bash
python3 deploy/installer/setup.py tailnet-login
```

The result reports installed/available state, current Tailnet, device, tags and,
for an untagged device only, the network account. Tagged machines do not identify
which person should administer Loom. Do not infer Admin from the OS account.

| State | Recommendation and action |
|---|---|
| Client missing | Install prerequisites with `deploy/tailnet.py install-client`; inspect again |
| Logged out | Confirm target Tailnet/account; run the bounded login command and let the user authenticate |
| Connected to the intended Tailnet | Recommend reuse; confirm that choice and verify the account when available |
| Connected to a different account/Tailnet | Offer reuse or explicit switch; never log out automatically |
| Device approval pending | Ask the Tailnet administrator to approve; re-inspect, never treat a login URL as success |
| Tagged machine | Verify Tailnet/device; collect Loom Admin independently |
| Daemon unavailable/stopped | Resolve daemon/connectivity and inspect again; do not replace credentials |
| Existing Loom installation | Choose update, recovery or deliberate removal; fresh apply refuses an existing instance |

```bash
# Read-only verification of the selected existing connection.
python3 deploy/installer/setup.py tailnet-login --login-action reuse --tailnet example.test

# No existing connection: show the real authentication URL, then return.
sudo python3 deploy/installer/setup.py tailnet-login --login-action login \
  --tailnet example.test --login-email admin@example.com

# Only after the user explicitly chooses account switching and has alternate access.
# This logs out the current connection and can disconnect SSH/Codex immediately.
sudo python3 deploy/installer/setup.py tailnet-login --login-action switch \
  --tailnet other.example --login-email other@example.com --confirm-disconnect
```

Authenticate as the selected account in the provider UI. After a disconnected
switch, continue from the local console/alternate channel. Re-run inspection and
`reuse` with the expected Tailnet (and `--login-email` for an untagged account).
The selected identity must match before generating a plan. Merely changing Loom
Admin does not require logging the machine out. No logout is part of uninstall.
An explicit switch starts a new user login without advertising the old device's
tags. Installation assigns the selected Tailnet's Loom tags only after verification.

Generate the installation plan only after enrollment is verified. Store management
credentials through `deploy/access_setup.py credential`, not chat or shell history.
Continue with the numbered installation steps and reread the index after each.
