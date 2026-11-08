# 2. Host and credentials

Required: Ubuntu/Debian systemd host, Python 3, Docker Engine + Compose, OpenSSL, acl, util-linux and Tailscale; Git for selected source clones. Install missing packages for the actual OS without replacing an existing Docker distribution. GPU requires compatible drivers and NVIDIA Container Toolkit. Offline media must already contain host packages and execution images.

The administrator logs the machine into the intended tailnet. Never automatically log out of another tailnet. Confirm authorization for `tag:loom` and machine-scoped Sharing policy. Never broaden private access to every future tailnet member. Tailscale login is not a management API credential.

Store the management API access token without printing it:

```bash
sudo python3 deploy/access_setup.py credential
sudo python3 deploy/installer/setup.py doctor /absolute/install-plan.json --stage host
```

In a kit the helper is `platform/deploy/access_setup.py`. A missing/expired credential, insufficient API permission, pending device approval or unsafe broad policy is an incomplete host step, not permission to install token-only mode. The agent explains the exact console action required and resumes after it; no repeated polling or invented success.

The installer only mounts a selected existing ext4/xfs filesystem. Never format, repartition, recursively chown an existing storage tree, or install unselected research environments. Use `mount: null` for an already mounted volume.

Exit: host doctor passes. Reread INSTALL.md and the step index before step 3.
