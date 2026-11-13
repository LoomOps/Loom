# Runtimes and dependency installation

A Runtime is an independently versioned execution environment: an immutable local
image ID, exact dependency inventory, resource/network/driver policy and measured
validation evidence. Components implement Job lifecycle; Tools implement reusable
research code. A Profile selects their exact revisions and a Runtime revision for
each stage. A Runtime is not generated implicitly whenever a Profile is registered.

## Discover and select

Call `components.runtimes` and `components.get_runtime` using the live schemas from
`system.describe`. Discovery returns only this executor instance's grants, exact
dependencies, recorded validation scope and consuming Profiles/Jobs. Installation
attempts report staging/building/validating/ready/failed independently of Jobs. Dashboard
Runtimes groups Profiles, Components, Tools and Runtimes with the same evidence and relationships.
`GET /api/runtimes` exposes the same authenticated read operation.

Use `runtime_ref: {"id":"simulation","revision":1}` instead of `runtime_id` or
`image` in a Profile runner. Loom assigns its hash/image/dependency pin. Callers
cannot supply `runtime_pin`. Submitted Jobs preserve the pin even when a newer
environment is installed. Legacy administrator `runtime_id` entries and direct
image Profiles remain supported; they do not become verified Runtime revisions.
Legacy Jobs retain their recorded provenance without inferred backfills.

Component and Tool manifests may declare `requires` as dependency name to exact
version string. Registration checks these against the selected run Runtime's
inventory. This is an exact-version check, not a dependency solver or a substitute
for testing custom code, physics, sensors and model behavior.

## Resolve the installation directory

Use the dependency directory selected when this Loom installation was configured.
The installer passes `plan.dependencies` to the matching host executor instance.
The administrator tool reads that setting and the saved installation plan; they
must agree. Existing installs can use their saved plan without replacing executor
configuration. If neither setting exists, report the missing configuration. Do
not infer a directory from another machine, assume `/dependencies`, follow a
redirected symlink or silently fall back to a home directory.

Managed source, verified downloads, recipes, locks and evidence are staged under
`<configured-dependencies>/installations/<runtime-id>/<installation-attempt>/`.
Its `context/dependencies/<name>` contains downloaded/extracted dependencies.
Python/system packages installed by the recipe reside inside the immutable image;
never install them into the Loom service's Python environment or the host's global
Python. Dataset/model outputs remain in registered data stores.

## Authorized administrator workflow

Loom MCP does not execute privileged builds, arbitrary installers or Docker commands.
When a user explicitly requests installation, a host-capable agent can use the
administrator workflow below. If it lacks host/Docker access, provide the concrete
plan to an authorized administrator instead of claiming installation is complete.
Merely connecting Loom Use never authorizes host installation.

Prepare a reviewed source directory containing `Dockerfile`, lock files and
`runtime.json`. Pin base-image/dependency versions and preserve patches. Example
manifest (replace the validation script with the actual environment's smoke):

```json
{
  "id": "simulation",
  "label": "Simulation environment",
  "dependencies": [{"name":"sim","version":"6.0.0","kind":"python"}],
  "runtime": {
    "cpus":8,"memory":"32g","pids":1024,"network":"none",
    "driver_capabilities":["compute","utility","graphics"]
  },
  "validation": {
    "command":["python3","/opt/validation/smoke.py"],
    "gpu":true,"timeout":300,"checks":["headless-launch","physics-step"]
  }
}
```

The validation command must emit a final single-line JSON object containing
`dependencies` (name to actual installed version) and named boolean `checks`.
All declared versions must match and every required check must be true. Use real
imports/version inspection and actual launch/physics/render operations; do not
print constants to manufacture evidence. Logs and their hash remain with the recipe.
Registration proves only the recorded command/check scope, not all future workloads.

For a downloaded dependency add `url` (HTTPS), `sha256` and optional `archive`
(`file`, `tar` or `zip`). Downloads are verified before extraction; escaping paths,
links and special archive members are rejected. URLs require a hostname and must
not contain user information, queries or fragments: Runtime metadata is visible
through the authenticated catalog. Pre-stage authenticated/private packages in the
reviewed recipe instead of embedding credentials or signed download URLs.

From the selected source/installation kit, use explicit target paths:

```bash
sudo python3 deploy/runtime.py plan --installation "$loom_instance" \
  --executor "$loom_executor" --instance "$executor_instance" --source "$recipe_source"
sudo python3 deploy/runtime.py install --installation "$loom_instance" \
  --executor "$loom_executor" --instance "$executor_instance" --source "$recipe_source"
```

`plan` is read-only. `install` stages dependencies, builds without overwriting an
existing image tag, runs bounded isolated validation, then registers a new Runtime
revision and grants it to the selected instance. The environment is discoverable
without restarting Ops/executor. GPU dispatch is held while installation/validation
owns its lock; existing GPU tickets must first return. CPU Jobs are not preempted.
Failed builds/smokes retain evidence and do not register a usable revision.

If validation-container return is unconfirmed, GPU dispatch stays fenced. Inspect
the attempt and use `reconcile --attempt <installation-id>` with the same target
arguments to remove only its recorded validation container and confirm return.
This does not retry compute or mark the failed installation successful.
The attempt must belong to the selected executor instance. Repeating reconciliation
after confirmed return performs no Docker cleanup. Existing ready revisions remain
ready; a registration whose final receipt was lost is identified from its exact
catalog entry rather than registered or built again.

List summaries omit large source/context hash maps; get_runtime returns the exact
manifest on demand. Runtime metadata lives in the executor's separate `runtimes.sqlite3`; Job history,
tokens and Profiles remain in their existing stores. Images remain in Docker;
dependency files remain under the configured directory. Updating platform images
does not rebuild or replace these environments. Preserve the catalog, directory
and image identities together in backup/migration; restoring metadata alone does
not restore packages. No framework-specific Isaac compatibility rule is hardcoded
in Loom: the environment owner pins a documented compatible Sim/Lab pair and tests it.

Existing environments can be inventoried with the administrator-only
`deploy/runtime_inventory.py` helper using the actual executor instance, runtime ID
and interpreter. It probes metadata without GPU/framework execution or installing
packages, records declared mount metadata, and preserves all old Profile/Job pins.
A measured current observation is distinct from historical execution proof. Review
its exact inventory scope before selecting it for a newly registered Profile.

## Representative dependencies and installation provenance

Installed inventory is an observation of packages bundled in an image or found in
configured mounts. It does not prove that every package was invoked or required by
a Job. Representative dependencies are explicit name/scope selectors declared by a
recipe or curated on an exact Runtime with components.curate. They never install,
remove or upgrade packages. Conflicting versions require an explicit scope.
Dashboard defaults to representative entries; complete inventories are available
only under Installed inventory, separated into Managed directory, Image, External
mounts, Deprecated and Unclassified. New inventory probes record canonical source
provenance using the configured dependency directory and flag deprecated engine
extensions. Missing provenance in older observations is not guessed.

Use components.inspect_profile before preparing a reusable submission, and
components.compare_profiles to check whether execution definitions differ. These
are read-only aids, not new Job gates. Component/Tool stage_requires can select
stage, name, exact version and optional dependency scope; flat requires retains
its main-stage semantics. Tool roles curated on a Profile are declarations, not
proof of calls. Pinned Jobs count distinct started logical Jobs; associations via
legacy IDs/current observations do not prove a historical exact Runtime revision.
