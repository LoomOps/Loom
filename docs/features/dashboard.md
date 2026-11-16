# dashboard — Dashboard HTTP relay·workspace UI

## Boundary

Never send credentials to the opaque iframe. Browser writes respect Origin/CSRF boundaries. The UI must not infer completed backend states.

Outputs uses compact typed field sections, without Overview prose. One Sources
disclosure contains original metadata, field evidence, file locations and
publication context. Dataset quantity labels preserve episodes/cases/snapshots;
recorded training eligibility is not a model-quality acceptance. Dataset grouping
follows explicit exact lineage, retains variant names and overlapping counts, and
does not infer a data-parent edge from Task names or general consumes relations.
Group counts describe loaded variants; paging does not claim a complete lineage.

The current native service is implemented in `rust/`. Python files in this block may be retained web/administrator integrations or compatibility references; they do not supersede the native transactional handlers. See [implementation boundaries](../IMPLEMENTATION.md).

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
- [dashboard/workspace/outputs.js](../../dashboard/workspace/outputs.js) — Outputs navigation and nested Comparison.
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

Run the affected suites and `deploy/features.py check`; see [the verification matrix](VERIFICATION.md). Passing retained compatibility fixtures is distinct from exercising the native handler.

Acceptance outside these fixtures:
- Actual Safari behavior, existing user sessions and SSO provider login require separate acceptance testing.


## Bounded catalogs and refresh

A bootstrap is only a seed. Same-revision refresh preserves loaded list pages and
parent-side detail identities. A new revision invalidates page cursors and pending
responses before the visible list reloads. Research history uses its selected
scope; current queue occupants remain visible in every presentation view.
Tasks appear as filters and record labels, without a standalone Scenario tab.
Explorer requests actual storage observations even with the Inspector closed and
loads registered file records independently of the research pane. Its bounded
pages are not a recursive filesystem inventory or an assertion that unregistered
files do not exist.

## Outputs navigation

The sidebar exposes Outputs with Datasets, Experiments and Models sub-tabs.
Comparison is reached inside Experiments and retains its selection when moving
back to results. Task selection carries across output sections and record links.
Legacy models/datasets/experiments/comparison view URLs and exact record links
remain valid; /views/outputs opens Datasets and Task entities remain addressable by exact links. This is presentation navigation,
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
between output sections. Queue/history badges preserve keyboard focus and theme
contrast. Scientific metadata precedes Owner; full recorded metadata remains
available below. Dataset role labels remain source metadata, not browse headings.

Output purpose filters separate Research and Development using declared roles,
engineering classifications and explicit Observer model roles. Main outputs sort
before supporting records. Observer weights remain Models; executable code uses
Tools or lifecycle Components. Development Jobs use category `development` and
normal resource admission; their current queue rows have a separate panel without
hiding resource occupants. Historical Jobs are not rewritten to invent a category.

A list emphasizes one unambiguous declared primary metric, one exact sole outcome
highlight, or one sole metric. Multiple primaries/populations have no synthesized
headline. All measurements and denominators remain expandable and in Comparison.
Measured physics, annotation, language conditioning, evaluation scope, protocol,
evaluator, population and thresholds are shown when recorded, never inferred from
an item name. Missing metadata is not fabricated from a benchmark example.

Index projection status appears in the lower sidebar. Its host software values
use the newest source observation, so an older catalog bootstrap cannot replace
a newer resource sample. Source timestamps remain source timestamps.

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
