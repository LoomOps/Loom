# dashboard — Dashboard HTTP relay·workspace UI

## Boundary

Never send credentials to the opaque iframe. Browser writes respect Origin/CSRF boundaries. The UI must not infer completed backend states.

The Catalog uses compact typed field sections. One Sources
disclosure contains original metadata, field evidence, file locations and
publication context. Dataset quantity labels preserve episodes/cases/snapshots;
recorded training eligibility is not a model-quality acceptance. Dataset grouping
follows explicit exact lineage, retains variant names and overlapping counts, and
does not infer a data-parent edge from Task names or general consumes relations.
Group counts describe loaded variants; paging does not claim a complete lineage.
Datasets, Policies and Experiments share the scientific list and filter styles.
The Dataset list uses the Task index and catalog class. Different splits of the
same variant can share its label and subgroup. Standard split counts appear in
the Split field as `100/20/20`. The separate Tag field displays declared labels
in monospace, aligned with adjacent facts. Each entry
keeps its exact link. The browser hides a legacy split suffix or
subgroup only when it matches the structured split and declared path. Dataset
origin badges use recorded provenance. A `dataset_import` report identifies a
legacy import only when the record has neither an explicit origin nor a producer
Job. Policies and Experiments have a Family
index. Selecting a Task also shows linked Datasets with their recorded splits in
the index. These filters intersect. Recognized
variants of a recorded architecture share a family label. Policies support text search.
Declared Observer models stay reachable by exact links and through discovery.
They are omitted from the Policies list.
The Policies and Experiments lists fold identical preserved copies of the same
source revision within the selected catalog context. Every exact snapshot
remains addressable, including copies whose original records differ.

The Catalog orders Datasets, Policies and Experiments. Its left Task index initially selects All and lists Tasks with outputs of the active kind, catalog class and disposition. The index covers results beyond the first page. Switching tabs or filters keeps a selected Task while it still has matching outputs. All includes unlinked records. Reporting-enabled details use
Overview with optional Distribution and Episodes for Datasets, Overview and
Training for Policies, and Overview, Results and Cases for Experiments.
Each Overview presents its metadata sections together in one standard panel,
including records without a reporting contract. Episode and Case selectors load additional members in place. Policy training groups charts in
panels. Equivalent summary and
report values are shown once. Base model shows the recorded name or identifier
without its digest. The original value stays in Sources. Exact metric definitions
remain available in each learning chart's disclosure.
Exact Template and Contract references live in the Overview footer. Reports
choose typed series, scatter, bar and coverage-matrix renderers. Distribution
charts identify their measured or assigned population and preserve paging.

## Bounded workspace reads

The shell advertises compact list support. The backend separates queue and
terminal history reads from scientific output and software lists. History
orders terminal records before returning each ten-row page. Exact references
stay addressable when a row carries only presentation fields. The Linux
Inspector uses a compact resource scope and computes its slot counts directly
from live execution state. The persistent sidebar uses the host scope, while
Explorer uses storage observation and indexed registered-directory pages.

Software inventories, pinned executions, output files and Training revisions
are read when their sections open. Browser refreshes use authority domain
versions for work, history, outputs, software, software usage and Tasks. They retain the last
complete view during a refresh. Search input is debounced and superseded
software queries can cancel their browser requests.

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Browser relay | `dashboard/server.py#WebHandler` | Allowed read route or queue mutation + authenticated browser | Bounded response; session/Origin/CSRF guard for writes; no generic arbitrary proxy | api |
| HTTP server | `dashboard/server.py#make_server` | Ops origin, optional SSO config and exact trusted ingress | Dashboard server with explicit authentication mode | installation |

## Sources and contracts

- [dashboard/.dockerignore](../../dashboard/.dockerignore) — integration, packaging or documentation.
- [dashboard/Dockerfile](../../dashboard/Dockerfile) — integration, packaging or documentation.
- [dashboard/branding/brand.css](../../dashboard/branding/brand.css) — integration, packaging or documentation.
- [dashboard/branding/brand.js](../../dashboard/branding/brand.js) — integration, packaging or documentation.
- [dashboard/branding/logo.svg](../../dashboard/branding/logo.svg) — integration, packaging or documentation.
- [dashboard/branding/scene.js](../../dashboard/branding/scene.js) — integration, packaging or documentation.
- [dashboard/dev.py](../../dashboard/dev.py) — integration, packaging or documentation.
- [dashboard/server.py](../../dashboard/server.py) — integration, packaging or documentation.
- [dashboard/theme/style.css](../../dashboard/theme/style.css) — integration, packaging or documentation.
- [dashboard/theme/theme.js](../../dashboard/theme/theme.js) — integration, packaging or documentation.
- [dashboard/workspace/access.html](../../dashboard/workspace/access.html) — integration, packaging or documentation.
- [dashboard/workspace/activity.js](../../dashboard/workspace/activity.js) — integration, packaging or documentation.
- [dashboard/workspace/build.py](../../dashboard/workspace/build.py) — integration, packaging or documentation.
- [dashboard/workspace/dependencies.js](../../dashboard/workspace/dependencies.js) — integration, packaging or documentation.
- [dashboard/workspace/details.js](../../dashboard/workspace/details.js) — integration, packaging or documentation.
- [dashboard/workspace/explorer.js](../../dashboard/workspace/explorer.js) — integration, packaging or documentation.
- [dashboard/workspace/extensions.js](../../dashboard/workspace/extensions.js) — integration, packaging or documentation.
- [dashboard/workspace/frame.css](../../dashboard/workspace/frame.css) — integration, packaging or documentation.
- [dashboard/workspace/frame.js](../../dashboard/workspace/frame.js) — integration, packaging or documentation.
- [dashboard/workspace/outputs.js](../../dashboard/workspace/outputs.js) — Outputs navigation and standalone Comparison.
- [dashboard/workspace/icons.js](../../dashboard/workspace/icons.js) — integration, packaging or documentation.
- [dashboard/workspace/job-activity.js](../../dashboard/workspace/job-activity.js) — integration, packaging or documentation.
- [dashboard/workspace/lazy_logs.py](../../dashboard/workspace/lazy_logs.py) — integration, packaging or documentation.
- [dashboard/workspace/mcp.js](../../dashboard/workspace/mcp.js) — integration, packaging or documentation.
- [dashboard/workspace/media.js](../../dashboard/workspace/media.js) — integration, packaging or documentation.
- [dashboard/workspace/operations-desk.css](../../dashboard/workspace/operations-desk.css) — integration, packaging or documentation.
- [dashboard/workspace/parent.css](../../dashboard/workspace/parent.css) — integration, packaging or documentation.
- [dashboard/workspace/parent.js](../../dashboard/workspace/parent.js) — integration, packaging or documentation.
- [dashboard/workspace/projection.js](../../dashboard/workspace/projection.js) — integration, packaging or documentation.
- [dashboard/workspace/queue.js](../../dashboard/workspace/queue.js) — integration, packaging or documentation.
- [dashboard/workspace/research.js](../../dashboard/workspace/research.js) — integration, packaging or documentation.
- [dashboard/workspace/surfaces.js](../../dashboard/workspace/surfaces.js) — integration, packaging or documentation.
- [dashboard/workspace/template.html](../../dashboard/workspace/template.html) — integration, packaging or documentation.
- [dashboard/workspace/text-preview.js](../../dashboard/workspace/text-preview.js) — integration, packaging or documentation.
- [dashboard/workspace/search.js](../../dashboard/workspace/search.js) — integration, packaging or documentation.
- [dashboard/workspace/catalog-pages.js](../../dashboard/workspace/catalog-pages.js) — integration, packaging or documentation.

## Change review

Rebuild the generated workspace after UI source edits. Verify both HTTP behavior and real Firefox for authentication, session or relay changes. Review the full installation doctor and step exit criteria when this feature changes; a local health response does not establish user-facing readiness.

The native resource response preserves the workspace's `host`, `observed_at` and
`sections` envelope. Host labels come from the recorded host observation, with
the installation machine ID retained separately and stale/missing samples marked.
Verify Models, Datasets, Experiments and Troubleshooting beyond the bootstrap and
first page; Models and Experiments have independent list renderers.

Related blocks: [identity](identity.md) · [api](api.md) · [catalog](catalog.md) · [observations](observations.md) · [onboarding](onboarding.md) · [diagnostics](diagnostics.md).

## Verification

- [tests/dashboard/browser/test_workspace.py](../../tests/dashboard/browser/test_workspace.py)
- [tests/dashboard/test_proxy.py](../../tests/dashboard/test_proxy.py)

Acceptance outside these fixtures:
- Actual Safari behavior, existing user sessions and SSO provider login require separate acceptance testing.


## Bounded catalogs and refresh

The browser opens the authenticated shell first. Visible Home panes then request
queue occupants, paged history and Activity independently. Job details request
the queue for controls and retain the last successful Unit list while reloading.
The visible queue reloads every five minutes while there are running Units.
It identifies the estimated phase and shows an unavailable state when usable
progress is missing.
Recent Job detail identities remain available between the shell and queue reads.
A transient detail failure keeps the previous result and offers retry. Explicit
denial or removal clears the old detail. Same-revision refresh preserves loaded
pages. A new revision invalidates page cursors and outdated list responses before
the visible list reloads. Dashboard history starts its paged Job
read when bootstrap data patches the pane. It waits for the first complete Job
page and keeps the previous page during refresh or partial reads. The queue
shows current user work independently of history pagination. Runtime lists with
no visible rows show loading or indexing until a complete response arrives.
An unloaded native deep link resolves its logical Item alias or exact native
entity, Job or Task identity before text search; it does not require the record to be among bootstrap or first-page rows.
Tasks appear as filters and record labels, without a standalone Scenario tab.
Explorer requests actual storage observations even with the Inspector closed and
loads registered file records independently of the research pane. Its bounded
pages are not a recursive filesystem inventory or an assertion that unregistered
files do not exist.

## Catalog navigation

The sidebar exposes Catalog with Datasets, Policies and Experiments sub-tabs.
Comparison is a separate sidebar view with a columns-3 icon. Its selections
survive navigation back to the Catalog. Catalog headers contain the three sub-tabs;
Templates and Contracts browsers are available from Loom Use. Task selection carries across output sections and record links.
Legacy models/datasets/experiments/comparison view URLs and exact record links
remain valid; both `/views/catalog` and `/views/outputs` open Datasets. Task entities remain addressable by exact links. This is presentation navigation,
not a new artifact kind or an automatic merge of scientific records.

The Runtimes sidebar and Runtime tab share server-cog. Tools use hammer; the
Troubleshooting wrench remains distinct.

Catalog revision refresh retains the last complete per-query list while replacing
all previously loaded pages atomically. Query membership owns list visibility;
unrelated cached DTOs cannot repopulate a removed/filtered row. Background patches
key rows by output identity and anchor the visible row at patch time, retaining
its offset when earlier rows move. Navigation still resets scroll normally.
Output titles keep their original labels; Task appears as a separate property.

Task is a filter and a compact badge above the original output title; there is no
Tasks sub-tab or New Task button. Task detail links retain context when moving
between output sections. Queue/history rows place subdued Task links directly
above Owner, after the Job title and execution metadata. Links preserve keyboard
focus and theme contrast. Scientific metadata precedes Owner; full recorded metadata remains
available below. Dataset role labels remain source metadata, not browse headings.

Catalog filters use Regular and Reference on Job/Unit installations, with
Regular selected by default. The legacy Research and Development filters are
available for older servers. Historical Observer models retain their recorded
role. New Observer evidence belongs to the producing Unit or the reusable Tool
or Component revision.

A list emphasizes one unambiguous declared primary metric, one exact sole outcome
highlight, or one sole metric. Multiple primaries/populations have no synthesized
headline. All measurements and denominators remain expandable and in Comparison.
Measured physics, annotation, language conditioning, evaluation scope, protocol,
evaluator, population and thresholds are shown when recorded, never inferred from
an item name. Missing metadata is not fabricated from a benchmark example.

Host software values use the newest source observation, so an older catalog
bootstrap cannot replace a newer resource sample. Source timestamps remain source
timestamps.

Presentation references: [robomimic dataset structure](https://robomimic.github.io/docs/datasets/overview.html)
separates environment arguments, observation/action arrays, trajectories and split
masks. [LIBERO](https://arxiv.org/abs/2306.03310) evaluates task suites rather than
an unlabeled aggregate. These inform information hierarchy only; the source
records supply all displayed scientific facts and their acceptance definitions.

Scientific cards and detail sections use the same `research_summary` facts, with
recorded zero/false values preserved. Datasets show population, split, modalities,
physics/control clocks and annotation/instruction context. Models show actual
inputs/predictions, initialization/selection and role-appropriate validation error;
Observers do not inherit VLA language/action assumptions. Experiments show the
population, evaluator and success thresholds alongside their declared primary
outcome. Raw metadata, artifact evidence and full metrics remain accessible below.
The UI does not infer physics from a name or convert a sensor diagnostic into task
success. `research-metadata.js` is presentation; extraction belongs to the Rust core.

The information hierarchy also follows [MimicGen](https://mimicgen.github.io/)
(source demonstrations, reset distributions and policy performance as distinct
quantities) and [OpenVLA evaluation details](https://arxiv.org/html/2406.09246v3#A2)
(robot/task/cohort, evaluation rubric and per-condition results). These are
presentation references, not measurements assigned to Loom records.

Explicit condition comparisons show Strict/Functional success counts over each
condition's valid population, plus planned counts. They are not pooled into a
headline rate. Check counters and counts greater than their population are not
task outcome measurements.

Scientific inputs use full-width responsive groups with signal, shape, units,
dtype and preprocessing only when recorded. Dataset observations and Model inputs
share this renderer. Detail summaries do not duplicate input tensors in both the
Model panel and the Inputs panel. Empty list facts do not reserve blank rows.
A same-generation metadata-only bootstrap retains verified detail/page facts;
an exact read or a new source generation can replace them explicitly.


Dataset browsing uses existing Task families, explicit nested subgroups, and a
compact multi-generation derivation tree. It preserves specific Dataset names,
count units, sampling/physics facts and exact cross-links. Collapsed groups and
branches survive catalog refresh. Other records remain available in the All view.
Catalog indexing uses an accessible icon-only spinner beside the footer update
control; it is hidden at idle and respects reduced-motion preferences.

## Metadata formats

Loom Use exposes a bounded template browser only when the server advertises
metadata_templates. Exact references can be copied for Profile/producer selection.
Profile details show their frozen formats. Robot context precedes scientific
sections, which render the same typed template fields as list facts; extension
fields remain escaped and bounded. Old servers retain their existing UI.

Legacy recorded metrics stay in the Metrics tab (Quality for Datasets), with their
graph/table control and original definitions intact. They do not repeat below
unrelated report tabs. Structured metric definitions render as escaped values,
and long labels wrap without forcing the pane wider. Comparison initially chooses
a model with an available evaluation; explicit user choices remain unchanged.

The current queue, terminal history and Home Activity include user work across
Research and Operations without a Job scope selector. System ownership identifies
platform work excluded from the Dashboard. Operations records the purpose of work
regardless of ownership. Terminal history shows Job and Standalone Unit in the
Type column before Status. The Unit Queue also labels standalone Units and links
Job-owned Units to their Job. Completed consolidation sources are visible through
their continuation. Presentation-only parent links preserve discoverability in
detail. A Pilot label requires both a named preflight purpose and a recorded
reduced scope. Exact Job records retain their original mode.

Activity responses from older revisions cannot overwrite the current observation.

An asynchronous bootstrap refresh must renew Activity for its new revision even
when no full render occurs. Keep one in-flight request per revision/view, ignore
older responses and retain the last verified rollup until replacement. On first
load, show a spinner until the Activity response is available, including when
it contains zero days. A failed read offers retry. Browser regression holds an
old reply across a bootstrap and requires the newer request without a filter
click or navigation.
