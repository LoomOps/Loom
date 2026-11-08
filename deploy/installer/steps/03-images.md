# 3. Release and installation kit

Choose a published GitHub Release, not unpublished main. Clone/check out its exact tag in a separate clean directory/worktree. Use that same release number for the kit build; do not reuse a numbered release for changed source.

```bash
sudo python3 deploy/source_install.py <release-version> --destination /var/tmp/loom-install-<release-version>
```

The builder explicitly prepares the Python execution image; BuildKit cache is not sufficient. It records image IDs and publishes the completed kit only after packaging. If images were built but packaging failed, rerun the same command with `--resume`: it verifies the original snapshot and immutable service image IDs before packaging. Never reconstruct manifest files by hand.

Offline: extract the checksum-verified kit and load its `images.tar` with `sudo docker load --input /absolute/kit/images.tar` before image diagnostics. All modes use the same complete service contract.

```bash
sudo python3 /absolute/kit/setup.py check /absolute/install-plan.json
sudo python3 /absolute/kit/setup.py doctor /absolute/install-plan.json --stage images
```

Exit: kit/plan checks and image doctor pass. Reread INSTALL.md and the step index before step 4.
