# 5. Verify services

```bash
sudo python3 /absolute/kit/setup.py doctor /absolute/install-plan.json --stage services
```

Doctor checks all three running service images, authentication bypass disabled, actual data mounts, Gateway secret isolation, native Dashboard entry, API/MCP, executor, startup enablement and selected GPU container visibility. A running container alone is insufficient. Service image IDs must match the kit, including Gateway's Dashboard image.

Every selected data/N store is RW. Doctor verifies RW bind mounts and access as the container service user; Job profiles can explicitly mount registered stores through the executor. Confirm all selected data/NAS mounts are available before restart. Research images are profile resources, not permanent extra Loom services.

A real CPU Job checks more than service health: submission, preparation, container execution, result finalization, registered artifact and resource return. Use the packaged smoke command in step 7; do not improvise a result schema. GPU visibility does not claim successful CUDA training. Record actual reboot recovery separately when performed, without rebooting a user's busy machine automatically.

Exit: services doctor passes. Reread INSTALL.md and the step index before step 6.

If host_ssh is selected, verify the protected host account service and its Unix
socket with ssh_setup.py doctor. Only Dashboard mounts the control socket; Ops
mounts readonly connection metadata; Gateway mounts neither. No personal account
is issued by installation. New plans enable the capability, not user access.

Compose startup waits up to 90 seconds for configured healthchecks. Doctor reports
health:starting as waiting, unhealthy as fail, and an absent smoke receipt as not_run.
Neither waiting nor not_run means ready. Run server doctor with sudo; client setup
remains unprivileged. SSH setup follows final executor storage registration.

Startup waits for the exact configured published host addresses, including a
Tailscale address that appears after network-online.target. This bounded address
wait is separate from the 90-second Compose health wait. Generated stack units
allow both phases and retry failed starts with backoff. Shutdown and read-only
Compose commands must remain usable while the address is missing. Do not widen a
private listener to 0.0.0.0 to work around boot ordering.
