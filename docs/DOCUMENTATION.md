# Documentation contract

## Audience and authority

| Document | Reader and purpose | Content to include | Authority |
|---|---|---|---|
| Tutorial | Person learning one supported workflow | Prerequisites, one bounded example, observable result | Tested workflow and live discovery |
| How-to | Person or agent with a specific task | Required decisions, operations, failure recovery | Current feature contract |
| Reference | Caller looking up exact behavior | Inputs, defaults, scopes, results, errors, identities | Native action registry and handlers |
| Explanation | Person learning the model | Relationships, tradeoffs, worked distinctions | Domain model and implementation |
| Feature specification | Maintainer changing the product | Owning boundary, consumers, invariants, tests, acceptance limits | `features/catalog.json` and executable sources |
| Agent instruction / Skill | Agent deciding what to load or do | Trigger, short routing, non-obvious task constraints | Installed client plus connected server |
| Historical design / audit | Maintainer investigating a decision | Original proposal, evidence and limits | The recorded revision; not current operating guidance |

A Skill is an entry point, not a copy of the manual. Put client installation and authentication in Loom-Use, server usage in `loom_use`, action inputs in the native registry, implementation boundaries in `docs/features`, and host installation in `INSTALL.md` / `deploy`. Keep one authority for each fact. Link to details rather than repeating them.

## Agent-facing writing

- Describe when a Skill applies in its frontmatter. Do not require loading every reference on every task.
- Assume the agent can program, read a schema and use ordinary tools. Add only Loom-specific information that changes a decision.
- Lead with the operation, its prerequisite and its observable outcome. Use exact action names, fields and IDs.
- Load the relevant guide and one action schema. Group-only `system.describe` provides that group's reference; a full reference is optional.
- Make conditions explicit: connection repair follows a failure or setup request; reconciliation follows an explicit asset scope; installation does not follow merely from reading a guide.
- Preserve the user's authorized scope. Do not introduce mandatory supervisors, approval rituals, global scans or unrelated setup.
- Keep non-obvious invariants near the operation: stable request keys, exact pins, resource-return evidence and immutable provenance. Avoid repeating a generic safety paragraph in every file.
- Separate a command to run from an illustrative JSON shape. Mark placeholders and the checks needed before a mutating example can run.
- Report unavailable tooling, stale indexes and unverified acceptance as limits. A schema-valid example is not proof of scientific correctness or a successful deployment.

Distributable prose is English. Intentional UI localization and multilingual fixtures remain unchanged. Do not include credentials, host catalogs, private evidence, measured research snapshots or machine-specific instructions.

## Change and verification

1. Locate ownership with `deploy/features.py impact <paths>` and read the handler and consumers.
2. Update the current document, schema examples and the affected client routing together. Historical records are preserved unless their status is misleading.
3. Build the native binary and regenerate the reference with `python3 deploy/documentation.py build --binary target/debug/loom-ops`.
4. Run `python3 deploy/documentation.py check --binary target/debug/loom-ops`, feature checks and affected tests. Run Loom-Use tests for client instruction changes.
5. Exercise discovery and the documented workflow against isolated native fixtures. Check failures as well as the successful response. Keep live-host and external-service acceptance separate.

`documentation-catalog.json` accounts for every tracked Markdown document and instruction surface. It records audience, status and authority, not a claim that every historical design is current. The check detects unclassified documents, broken local links/headings, missing guide coverage, stale generated references and schema-invalid action examples. Tests additionally verify guide cache invalidation and native HTTP/MCP behavior. These checks supplement code and prose review; they do not measure writing quality or grant permission to run production Jobs.

## Basis

The structure follows [Diátaxis](https://diataxis.fr/start-here/): tutorials, how-to guides, reference and explanation serve different needs. The compact entry point and selective loading follow [OpenAI's skill guidance](https://learn.chatgpt.com/docs/build-skills) and [skill-creator](https://github.com/openai/codex/blob/main/codex-rs/skills/src/assets/samples/skill-creator/SKILL.md). The [Agent Skills specification](https://agentskills.io/specification) defines portable metadata and progressive disclosure. [Anthropic's Agent Skills explanation](https://www.anthropic.com/research/equipping-agents-for-the-real-world-with-agent-skills) illustrates loading instructions and resources only when needed. Apply these principles to Loom's actual contracts rather than adding generic advice to every guide.
