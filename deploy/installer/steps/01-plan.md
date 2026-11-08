# 1. Inspect and recommend

Run `python3 deploy/installer/setup.py inspect` from the source checkout. In a kit use `python3 setup.py inspect`. If prerequisites/login are missing, complete the relevant host preparation in step 2 and return here.

Recommend a new output subdirectory on an existing mounted data volume, `/dependencies` for selected editable sources, and GPU only if hardware/driver/container support is present. Explain these recommendations using detected facts. Ask only for unresolved material choices: the login email of the Tailscale account that will administer Loom, which existing stores to expose, GPU needs and selected external source revisions. Ask explicitly: “Which Tailscale account will administer Loom? Provide that account’s Tailscale login email.” A contact email or OS username is not sufficient. Do not infer the Admin from the OS user or a tagged machine's owner. No research framework is a Loom prerequisite.

Generate the plan (replace the email with the confirmed Admin; add `--gpu` when selected):

```bash
python3 deploy/installer/setup.py plan --output /absolute/install-plan.json --admin-email admin@example.com --data /storage/loom/data
```

Use repeated `--additional-data /existing/path` for additional **read-only Explorer/catalog stores**. Job outputs and the executor use the primary `--data` directory; additional stores do not silently become Job input mounts. The generator reads this machine's real Tailscale identity, generates a stable installation identity and writes its linked Access plan. It never overwrites an existing plan. Confirm the proposed Docker subnet is unused and the selected ports are free. Never copy example machine addresses/identity placeholders unchanged.

```bash
python3 deploy/installer/setup.py doctor /absolute/install-plan.json --stage plan
```

Exit: the chosen settings are explicit and plan doctor passes. Reread INSTALL.md and the step index before step 2.
