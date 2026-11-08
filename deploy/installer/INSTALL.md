# Loom installation steps

Read the current step, run its script, then **reread this index and the repository INSTALL.md** before continuing. Inside a kit, this index is INSTALL.md; the original repository instructions are also under platform/INSTALL.md (source kit) or source/INSTALL.md (offline kit).

| Step | Document | Completion evidence |
|---|---|---|
| 1 | [Plan](steps/01-plan.md) | Valid full-service plan; documented recommendations and selected storage/Admin |
| 2 | [Host](steps/02-host.md) | Doctor `host` passes; actual tailnet and management API access |
| 3 | [Images](steps/03-images.md) | Kit checksums and doctor `images` pass |
| 4 | [Configuration](steps/04-configuration.md) | Doctor `configuration` passes |
| 5 | [Services](steps/05-services.md) | Doctor `services` passes |
| 6 | [Network](steps/06-network.md) | Doctor `network` passes |
| 7 | [Handoff](steps/07-handoff.md) | Full doctor and scripted handoff; live-test limits disclosed |

Mandatory everywhere: Ops, Dashboard, separate Gateway, native Tailscale user identity, Admin binding, machine Sharing, executor, selected storage, observations and startup. No token-only, IdP or auth-bypass variant. CPU versus GPU and storage choices vary by hardware; the service contract does not.

`setup.py inspect` recommends choices. `setup.py plan` generates the linked files. `check` checks plan/kit integrity. `apply` performs the ordered install and records completed stages. `doctor` observes actual state and exits nonzero on failure. `handoff` reruns doctor and refuses a success handoff if anything is missing. `resume-network` recovers publication using the same plan after configuration/services are healthy; it never recreates the database/executor.

Source checkout: run `python3 deploy/installer/setup.py ...`. Kit: run `sudo python3 /absolute/kit/setup.py ...`. Keep the original plan and protected credential file available for later diagnostics. Never rerun fresh `apply` over an existing or partially installed instance; inspect doctor and the saved progress first. Do not manually edit completion receipts.

## Removal

Uninstallation is separate from these installation steps. Use `platform/deploy/uninstall.py` (source kit) or `source/deploy/uninstall.py` (offline kit), with its adjacent `docs/UNINSTALL.md`. Preview the plan first; never delete management credentials or request records before remote Sharing cleanup succeeds.
