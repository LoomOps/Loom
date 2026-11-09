# Administrator runtime policy and opt-in checks

Components and Tools describe software; Profiles bind execution contracts. A
registered Profile, cached image or exit code alone is not runtime acceptance.
Keep immutable historical revisions and import receipts as evidence. Port host
launch assumptions into a new revision before treating old research code as a
portable Job entrypoint. Keep exact image/source/asset hashes in fresh acceptance
receipts. Never substitute an offline evaluator for a closed-loop evaluator.

## Administrator settings

The protected executor config owns `network` and `driver_capabilities`. Named
compatibility runtimes carry these in `runtimes.<id>`. For registered image
Profiles, the instance's `declared_runtime` selects these settings. Defaults remain
`network: "none"` and `driver_capabilities: ["compute", "utility"]`. The supported
network choices are `none` and ordinary Docker `bridge` egress. Host networking,
secret mounts and arbitrary Docker flags are not implicit grants. Allowing egress
does not install dependencies or replace pinned versions/caches.

Graphics workloads need an explicit administrator selection such as
`["compute", "utility", "graphics"]`. The accepted list is defined by
`executor/runtime_policy.py` and its `OPTIONS_SCHEMA`; `all`, empty/duplicate lists
and unknown names fail. Jobs cannot supply these fields as execution overrides.
Job environment values cannot override the broker's driver-capability selection.
Non-root UID/GID, read-only rootfs, capability dropping, mount grants and GPU
ownership checks still apply with bridge or graphics selected.

Fresh installation plans accept optional `runtime_options` with these two fields.
`executor-attach.py --network bridge --driver-capability compute
--driver-capability utility --driver-capability graphics` grants them to the new
instance's declared-image policy. It does not rewrite existing named runtimes.
The installation doctor compares an explicit plan to its actual matching grant.
Existing plans without the field keep their existing runtime settings.

Administrators review edits to existing protected configuration at an idle
maintenance point and restart the executor only after tickets return. Platform
updates replace helper code while preserving configuration bytes, mode and owner.
Unexpected config changes block update completion/resume/rollback for review.
Inspect local helper patches before updating; port required changes into product
source rather than relying on an untracked live-file hotfix.

## Bounded runtime doctor

The default command only inspects settings. `--run` explicitly starts one bounded
probe through the actual private executor. It uses the selected runtime's image,
mounts, UID/GID, network and GPU policy, rather than testing paths as host root.

```sh
sudo python3 deploy/runtime_doctor.py --root /loom/ops --runtime <runtime-id>
sudo python3 deploy/runtime_doctor.py --root /loom/ops --runtime <runtime-id> \
  --run --gpu --path /path/inside/container --timeout 60
```

The probe checks execution identity, mount/path readability, rootfs read-only
state, writable scratch, and selected driver libraries. It runs no inference,
scene, policy action or scientific evaluation and submits no research Job.
Python 3 must exist in the selected image. GPU is opt-in; an occupied GPU returns
pending without preempting its owner. This check is not a queue/admission gate.

Private JSON evidence is saved under `<instance>/diagnostics/runtime-doctor`.
Each run has its own executor ticket. On timeout only that ticket is cancelled;
workspace removal waits for confirmed return. Uncertain start/cleanup is retained
with its ticket and workspace for inspection and is never blindly replayed.
Exit 0 without valid probe evidence is a diagnostic failure.

Driver/path acceptance does not replace the user's separate GPU/Sim smoke or
full SDG/training/evaluation acceptance. Application wrappers must preserve Python
exception evidence before native shutdown and require expected output artifacts.
Core finalization still rejects missing manifests/files even when a process exits
0; do not weaken that boundary to make a migration appear successful.
