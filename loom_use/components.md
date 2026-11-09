# Components, profiles and dependencies

A component is an immutable revision of user source. A profile is an execution contract binding that revision to an image, commands, inputs, outputs, defaults and limits. Use components.write to edit, components.register to register, and components.register_profile for standalone profiles. Read each action's exact fields through system.describe. New components do not require new MCP servers or tools.

The runner receives a request JSON path as its final argument. The request includes params, output_dir, profile and Job context. Optional prepare/finalize/on_error/on_complete/on_cancel commands use the same runner format. External callbacks must deduplicate effects by event_id. Callback failures are reported separately from execution outcomes.

## Outputs

Write files and result.json into output_dir. The minimum result contains entities/metrics; entities include id, kind, path, provenance and relations. Record request.profile.revision and request.profile.sha256 as provenance.profile and provenance.producer_sha256 respectively. Declare domain semantics such as measurements, units, denominators and data splits in the profile's outputs/metadata_schemas. Generic Job state transitions do not generate domain metadata automatically.

## Installation and reuse

- `/dependencies/<repository>`: editable external source checkouts.
- `/dependencies/installations/<dependency>/<revision>`: Dockerfiles, patches, constraints/locks, build and validation records.
- Python environments and installed libraries: inside pinned execution images.
- `/loom/components`, `/loom/profiles`: registered user source and execution contracts.

Docker layer/build caches reduce installation work but do not preserve corrected source. Record local changes as recipes/patches, build a new image and select it in a new profile revision. Never overwrite existing image tags or active revisions. Source and recipes must remain sufficient to rebuild after cache deletion.

Standalone image profiles currently require an administrator-approved image already available in local Docker. Registration pins its digest. Jobs cannot request arbitrary host mounts, Docker sockets or privileged execution. The host executor owns network and image provisioning policy. Do not assume prepare has unrestricted Internet access.

External dependency checkouts and image recipes remain user-managed. Loom does not install research frameworks implicitly. Keep datasets, assets and weights in configured data storage.

## Writable data stores

`system.status.data_stores` lists configured stores and their container paths. New installations default host `/loom/data` to `data/0`; additional directories are RW stores such as `data/1`. A profile runner can declare `"storage": ["data/1"]` on runtime, prepare or a lifecycle hook. The executor mounts only those registered names at `/loom/data/1`, and the runner receives `request.storage["data/1"]`. No host path is accepted in this declaration. Unknown stores fail registration/execution.

For example: `"runtime": {"executor":"container","runtime_id":"python","command":["python3","/component/run.py"],"storage":["data/1"]}`. Standalone profiles expose the same field through HTTP and MCP. Omit storage for only `/work` plus declared read-only artifact inputs. Explicit RW access can modify shared files; coordinate changes to existing datasets. Publish result.json and registered outputs through `request.output_dir` on the Job’s selected `output_storage`; ordinary shared files in additional stores remain visible in Explorer. Storage mounts do not relocate historical artifacts or change their hashes/locators.

## Owner

Registration defaults to the authenticated user. Use `owner: "built-in"` explicitly
for common components; their display label is **Built-in**. Shared visibility alone
does not make a user component Built-in. `components.list` returns every namespace
unless filtered with owner. Results include owner_profile (id/login/display_name),
owner_name and owner_kind; profile discovery carries the same responsibility.
The existing owner input/output remains the stable namespace used by get/write and
profile references. Existing responsibility records take precedence for display;
paths, source pins and submitted Jobs are unchanged. Unknown historical identities
remain visible by their recorded ID, not guessed or relabeled Built-in.
