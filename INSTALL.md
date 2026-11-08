# Install Loom

Install the complete Loom service: **Ops + Dashboard + Gateway + host executor**, native Tailscale identity and machine Sharing. There is no reduced/token-only installation. A user's machine-specific identities, credentials and storage are never copied from another host.

For an existing instance, use [UPDATES.md](docs/UPDATES.md); do not rerun fresh installation. For new installations, follow the [step index](deploy/installer/INSTALL.md). Read only the current step and its references, then return to this index after the step.

1. **Plan** — inspect the host, default data/0 to host `/loom/data`, ask about additional RW storage and separate disk mounts, recommend GPU choices, confirm the Admin’s Tailscale login email, select/reuse the machine’s Tailscale connection, and generate linked configuration.
2. **Host** — prepare prerequisites, Tailscale enrollment and protected management credentials.
3. **Images** — select a published release and build/load its matching installation kit.
4. **Configuration** — apply the plan, native identity, Gateway and selected storage.
5. **Services** — verify the actual images, mounts, executor and startup units.
6. **Network** — verify private routes, public Gateway isolation and Sharing policy.
7. **Handoff** — run the complete doctor and give the **Gateway URL first**.

**After every step:** reread this file and the step index; compare the step's exit criteria with script output. Fix or report the missing condition before claiming that step is complete. A progress receipt is not proof of current health. Genuine administrator login/policy decisions may require the user; ordinary authorized installation steps do not need repeated permission.

Start with `python3 deploy/installer/setup.py inspect`. Recommendations must use its actual findings. Use `setup.py doctor PLAN --stage STAGE` for step checks and `setup.py doctor PLAN` for the complete report. Doctor is read-only; `verify` is an alias for the same complete diagnostics. See the step documents for the exact source/kit commands.

Do not substitute an owner token, a hand-written test result or an older UI for a failed installation step. Do not print credentials. Unperformed external-client/reboot checks remain explicitly unverified. Release discovery checks for updates; it never installs them automatically. After a successful handoff, offer Loom Use skill installation and MCP auto-registration in the user’s chosen agent client; follow step 7 and report client verification separately.

For removal, use the separate [uninstall workflow](docs/UNINSTALL.md). It cleans up Loom-managed Sharing/access records before deleting local authentication state; do not remove credentials first.

Connection/account scenarios: [Tailscale setup](docs/TAILSCALE-SETUP.md). Policy ownership and orphan repair: [managed resources](docs/MANAGED-RESOURCES.md).
