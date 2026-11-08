# 5. Verify services

```bash
sudo python3 /absolute/kit/setup.py doctor /absolute/install-plan.json --stage services
```

Doctor checks all three running service images, authentication bypass disabled, actual data mounts, Gateway secret isolation, native Dashboard entry, API/MCP, executor, startup enablement and selected GPU container visibility. A running container alone is insufficient. Service image IDs must match the kit, including Gateway's Dashboard image.

Every selected data/N store is RW. Doctor verifies RW bind mounts and access as the container service user; Job profiles can explicitly mount registered stores through the executor. Confirm all selected data/NAS mounts are available before restart. Research images are profile resources, not permanent extra Loom services.

A real CPU Job checks more than service health: submission, preparation, container execution, result finalization, registered artifact and resource return. Use the packaged smoke command in step 7; do not improvise a result schema. GPU visibility does not claim successful CUDA training. Record actual reboot recovery separately when performed, without rebooting a user's busy machine automatically.

Exit: services doctor passes. Reread INSTALL.md and the step index before step 6.
