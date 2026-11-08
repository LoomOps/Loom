# 1. Inspect and recommend

Run `python3 deploy/installer/setup.py inspect` from the source checkout. In a kit use `python3 setup.py inspect`. If prerequisites/login are missing, complete the relevant host preparation in step 2 and return here.

Recommend **host `/loom/data` as `data/0`**. Ask explicitly: “Keep `/loom/data` as data/0? Are there additional RW directories to register, or existing disks/filesystems to mount at another location?” Inventory disks and current mounts first; never select, format or repartition a disk automatically. A separate disk is optional. Use `/dependencies` for selected editable sources and recommend GPU only from detected hardware/driver/container support.

Ask for the login email of the Tailscale account that will administer Loom: “Which Tailscale account will administer Loom? Provide that account’s Tailscale login email.” A contact email or OS username is not sufficient. Do not infer the Admin from the OS user or a tagged machine's owner. No research framework is a prerequisite.

Generate the plan (replace the email with the confirmed Admin; add `--gpu` when selected):

```bash
python3 deploy/installer/setup.py plan --output /absolute/install-plan.json --admin-email admin@example.com --data /loom/data
```

Use repeated `--additional-data /storage/loom/data` to register **RW** stores as `data/1`, `data/2`, and so on. Directories may already exist or be created during installation; preserve existing files and ownership, granting the Loom service account access with ACLs. Host and container paths are distinct: host `/loom/data` maps to container `/loom/data/0`, and host `/storage/loom/data` maps to `/loom/data/1`.

For an explicitly selected, currently unmounted ext4/xfs filesystem, add `--mount UUID:/storage:ext4` (repeat for separate volumes). The target must be empty and contain at least one selected store. Existing mounted filesystems need no mount action. Review the generated `mounts` list before apply. The old single `mount` field remains accepted for existing plans.

The plan generator reads this machine's Tailscale identity and never overwrites an existing plan. Confirm the proposed Docker subnet and ports. Never copy example machine identities. Job profiles select RW stores with `runtime.storage` (also available on prepare/hooks); `system.status.data_stores` lists them. Job submission selects `output_storage` (default `data/0`) for the workspace and registered outputs. Additional profile `storage` mounts provide optional shared RW access at `/loom/data/N`.

```bash
python3 deploy/installer/setup.py doctor /absolute/install-plan.json --stage plan
```

Exit: the chosen settings are explicit and plan doctor passes. Reread INSTALL.md and the step index before step 2.
