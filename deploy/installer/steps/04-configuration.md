# 4. Apply configuration

```bash
sudo python3 /absolute/kit/setup.py apply /absolute/install-plan.json
sudo python3 /absolute/kit/setup.py doctor /absolute/install-plan.json --stage configuration
```

`apply` creates the non-login `loom:loom` identity without sudo/Docker group membership, persistent `/loom` layout, selected data mount, source dependencies, host executor, observations, native Dashboard identity, private approval worker, separate Gateway and Compose startup. It then publishes the private/public routes and runs doctor. Services may already be started when this command returns; steps 5 and 6 independently inspect them.

`/loom` holds host-owned configuration, DB, credentials, components/profiles and local overrides; images contain code and default guides. Existing user state is not a template to copy. The public Gateway receives request state but no management credential, SSO signing key or operational API relay. IdP is not installed.

On failure inspect install-progress.json and doctor. Completed local changes are not rolled back by deleting data. Do not repeat fresh apply into existing paths. `resume-network` is available only for a matching plan after configuration and services pass. Failures before that point require repair of the specific failed stage, not a fresh installation over the database.

Exit: configuration doctor passes. Reread INSTALL.md and the step index before step 5.
