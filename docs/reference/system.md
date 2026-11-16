# system

Installation status, contracts and guides

The examples are argument shapes, not live instructions. Replace symbolic identities and hashes with discovered exact records and satisfy the handler preconditions before a mutation. Nested schema constraints are retained below; response notes are descriptive, not a formal response schema.

## Actions

- [ssh](#ssh)
- [users](#users)
- [status](#status)
- [updates](#updates)
- [check_updates](#check_updates)
- [docker](#docker)
- [contract](#contract)
- [describe](#describe)
- [guide](#guide)
- [prune_search](#prune_search)
- [retry_catalog_task](#retry_catalog_task)
- [rebuild_search](#rebuild_search)
- [catalog_task](#catalog_task)
- [dashboard_page](#dashboard_page)
- [search_status](#search_status)
- [dashboard_summary](#dashboard_summary)
- [dashboard_activity](#dashboard_activity)
- [dashboard_entity](#dashboard_entity)

## ssh

Read the current caller SSH grant and machine addresses. Same-host clients use local files; Admin enablement is separate.

Scope: `read`. Owner: [api](../features/api.md). Implementation: [rust/crates/core/src/query.rs#system_action](../../rust/crates/core/src/query.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {},
  "required": [],
  "type": "object"
}
```

</details>

### Example arguments

```json
{}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Connection · state, host, port, username, authentication, home, addresses

Not enabled, unavailable or SSH management not configured.

## users

List registered user profiles available for ownership assignment.

Scope: `read`. Owner: [api](../features/api.md). Implementation: [rust/crates/core/src/query.rs#system_action](../../rust/crates/core/src/query.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {},
  "required": [],
  "type": "object"
}
```

</details>

### Example arguments

```json
{}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

User[] · id, provider, login, display_name

Shared authentication, schema, scope and handler preconditions still apply.

## status

Read build, contract revision, admission, registered runtimes and GPU execution availability.

Scope: `read`. Owner: [api](../features/api.md). Implementation: [rust/crates/core/src/query.rs#system_action](../../rust/crates/core/src/query.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {},
  "required": [],
  "type": "object"
}
```

</details>

### Example arguments

```json
{}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Status · build, api_revision, admission, inventory, runtime_ids, gpu_execution_enabled

Shared authentication, schema, scope and handler preconditions still apply.

## updates

Compare cached GitHub Releases with the current build. Reads do not wait on network requests or the Job database.

Scope: `read`. Owner: [api](../features/api.md). Implementation: [rust/crates/core/src/query.rs#system_action](../../rust/crates/core/src/query.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {},
  "required": [],
  "type": "object"
}
```

</details>

### Example arguments

```json
{}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

ReleaseCheck · status, installed, latest, update_available, checked_at, last_success_at, next_check_at, error, stale

Shared authentication, schema, scope and handler preconditions still apply.

## check_updates

Request a background check respecting the 60-second cooldown and GitHub rate limits. Does not install releases or stop Jobs.

Scope: `read`. Owner: [api](../features/api.md). Implementation: [rust/crates/core/src/query.rs#system_action](../../rust/crates/core/src/query.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {},
  "required": [],
  "type": "object"
}
```

</details>

### Example arguments

```json
{}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Receipt · accepted, reason, updates

Failures appear in updates.error while preserving the last successful result.

## docker

Read permitted Docker containers and observation status.

Scope: `read`. Owner: [api](../features/api.md). Implementation: [rust/crates/core/src/query.rs#system_action](../../rust/crates/core/src/query.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {},
  "required": [],
  "type": "object"
}
```

</details>

### Example arguments

```json
{}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Docker status · status, engine, containers, errors

Missing or stale observations are reported through status and errors.

## contract

Read Job properties, public states, recovery policy and submission schema.

Scope: `read`. Owner: [onboarding](../features/onboarding.md). Implementation: [rust/crates/core/src/query.rs#system_action](../../rust/crates/core/src/query.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {},
  "required": [],
  "type": "object"
}
```

</details>

### Example arguments

```json
{}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Contract · revision, submit_schema, status_values, recovery_policy

Shared authentication, schema, scope and handler preconditions still apply.

## describe

Omit group/action for the full reference; supply group for that group reference, or group and action for one exact action schema and example.

Scope: `read`. Owner: [onboarding](../features/onboarding.md). Implementation: [rust/crates/core/src/query.rs#system_action](../../rust/crates/core/src/query.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `action` | no | `"string"` | `—` |
| `group` | no | `["jobs", "recovery", "events", "components", "results", "system"]` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "description": "Omit both fields for the full scope-filtered reference; provide group for that group reference, or group and action for one exact action schema and example.",
  "properties": {
    "action": {
      "type": "string"
    },
    "group": {
      "enum": [
        "jobs",
        "recovery",
        "events",
        "components",
        "results",
        "system"
      ],
      "type": "string"
    }
  },
  "required": [],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "action": "submit",
  "group": "jobs"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Documentation index or Action reference · schema, scope, returns, errors, examples

Unknown action or action outside current scopes.

## guide

Read server version and guide index, or a document by id. Matching known_revision returns not_modified.

Scope: `read`. Owner: [onboarding](../features/onboarding.md). Implementation: [rust/crates/core/src/query.rs#system_action](../../rust/crates/core/src/query.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | no | `"string"` | `—` |
| `known_revision` | no | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "type": "string"
    },
    "known_revision": {
      "type": "string"
    }
  },
  "required": [],
  "type": "object"
}
```

</details>

### Example arguments

```json
{}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Guide · manifest, documents or content

Unknown document ID or invalid arguments.

## prune_search

Remove at most eight retired private shadow generations older than ten minutes. Preserve active generation and all authority.

Scope: `submit`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/maintenance.rs#maintenance](../../rust/crates/core/src/maintenance.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `key` | yes | `"string"` | `—` |
| `max_generations` | no | `"integer"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "key": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "max_generations": {
      "maximum": 8,
      "minimum": 1,
      "type": "integer"
    }
  },
  "required": [
    "key"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "key": "retired-search-cleanup",
  "max_generations": 4
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Durable maintenance progress or an explicit conflict.

task_stale task_state busy index_budget metadata_space

## retry_catalog_task

Resume a failed task with retained progress and unchanged configured roots. An exhausted original batch budget requires a new rebuild request.

Scope: `submit`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/maintenance.rs#maintenance](../../rust/crates/core/src/maintenance.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    }
  },
  "required": [
    "id"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "id": "task-id"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Durable maintenance progress or an explicit conflict.

task_stale task_state busy index_budget metadata_space

## rebuild_search

Build a separate source-qualified generation while the current index stays readable; switch atomically after catch-up.

Scope: `submit`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/maintenance.rs#maintenance](../../rust/crates/core/src/maintenance.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `key` | yes | `"string"` | `—` |
| `max_batches` | no | `"integer"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "key": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "max_batches": {
      "maximum": 100000,
      "minimum": 1,
      "type": "integer"
    }
  },
  "required": [
    "key"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "key": "rebuild-index-1",
  "max_batches": 10000
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Bounded typed JSON; partial coverage, stable replay receipts and explicit source conflicts.

source_conflict plan_stale validation_required content_changed retention_gap cursor_expired index_unavailable

## catalog_task

Read caller-scoped durable background maintenance progress, failure or receipt.

Scope: `read`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/maintenance.rs#maintenance](../../rust/crates/core/src/maintenance.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    }
  },
  "required": [
    "id"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "id": "task-id"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Bounded typed JSON; partial coverage, stable replay receipts and explicit source conflicts.

source_conflict plan_stale validation_required content_changed retention_gap cursor_expired index_unavailable

## dashboard_page

Project only selected metadata records into browser DTOs using the same filters and cursor as item search.

Scope: `read`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/catalog.rs#dashboard_page](../../rust/crates/core/src/catalog.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `at_least` | no | `"string"` | `—` |
| `availability` | no | `["verified", "source_unavailable", "recorded"]` | `—` |
| `category` | no | `"string"` | `—` |
| `cursor` | no | `"string"` | `—` |
| `dependency_ref` | no | `"object"` | `—` |
| `history` | no | `["current", "all"]` | `—` |
| `kinds` | no | `"array"` | `—` |
| `limit` | no | `"integer"` | `—` |
| `native_kind` | no | `"string"` | `—` |
| `origin_type` | no | `"string"` | `—` |
| `parent_item_id` | no | `"string"` | `—` |
| `recorded_from` | no | `"number"` | `—` |
| `recorded_to` | no | `"number"` | `—` |
| `related_to` | no | `"object"` | `—` |
| `role` | no | `["all", "main", "support", "test"]` | `—` |
| `scenario_ref` | no | `"object"` | `—` |
| `text` | no | `"string"` | `—` |
| `timeout_ms` | no | `"integer"` | `—` |
| `view` | no | `["all", "research", "operations"]` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "at_least": {
      "pattern": "^[0-9]{1,20}$",
      "type": "string"
    },
    "availability": {
      "enum": [
        "verified",
        "source_unavailable",
        "recorded"
      ]
    },
    "category": {
      "maxLength": 128,
      "type": "string"
    },
    "cursor": {
      "maxLength": 4096,
      "type": "string"
    },
    "dependency_ref": {
      "type": "object"
    },
    "history": {
      "enum": [
        "current",
        "all"
      ]
    },
    "kinds": {
      "items": {
        "type": "string"
      },
      "maxItems": 16,
      "type": "array"
    },
    "limit": {
      "maximum": 200,
      "minimum": 1,
      "type": "integer"
    },
    "native_kind": {
      "maxLength": 128,
      "type": "string"
    },
    "origin_type": {
      "maxLength": 128,
      "type": "string"
    },
    "parent_item_id": {
      "pattern": "^[1-9][0-9]{0,38}$",
      "type": "string"
    },
    "recorded_from": {
      "type": "number"
    },
    "recorded_to": {
      "type": "number"
    },
    "related_to": {
      "additionalProperties": false,
      "properties": {
        "item_id": {
          "pattern": "^[1-9][0-9]{0,38}$",
          "type": "string"
        },
        "relation": {
          "maxLength": 128,
          "type": "string"
        }
      },
      "required": [
        "item_id"
      ],
      "type": "object"
    },
    "role": {
      "enum": [
        "all",
        "main",
        "support",
        "test"
      ]
    },
    "scenario_ref": {
      "additionalProperties": false,
      "properties": {
        "id": {
          "maxLength": 2048,
          "minLength": 1,
          "type": "string"
        },
        "revision": {
          "minimum": 1,
          "type": "integer"
        }
      },
      "required": [
        "id",
        "revision"
      ],
      "type": "object"
    },
    "text": {
      "maxLength": 1024,
      "type": "string"
    },
    "timeout_ms": {
      "maximum": 1000,
      "minimum": 0,
      "type": "integer"
    },
    "view": {
      "enum": [
        "all",
        "research",
        "operations"
      ]
    }
  },
  "required": [],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "kinds": [
    "profile"
  ],
  "limit": 50
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Bounded typed JSON; partial coverage, stable replay receipts and explicit source conflicts.

source_conflict plan_stale validation_required content_changed retention_gap cursor_expired index_unavailable

## search_status

Read safe generation/checkpoint and partial coverage status without filesystem paths or private diagnostic content.

Scope: `read`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/index_store.rs#search_status](../../rust/crates/core/src/index_store.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {},
  "required": [],
  "type": "object"
}
```

</details>

### Example arguments

```json
{}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Bounded typed JSON with exact IDs; explicit errors or partial coverage when unavailable.

unknown_item index_unavailable cursor_expired source_unavailable content_changed query_too_broad

## dashboard_summary

Read compact Item counts and index capabilities; does not expand file or log inventories.

Scope: `read`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/items.rs#dashboard_summary](../../rust/crates/core/src/items.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {},
  "required": [],
  "type": "object"
}
```

</details>

### Example arguments

```json
{}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Bounded typed JSON with exact IDs; explicit errors or partial coverage when unavailable.

unknown_item index_unavailable cursor_expired source_unavailable content_changed query_too_broad

## dashboard_activity

Read a compact KST activity rollup from actual execution intervals and eligible registration times; no full catalog expansion.

Scope: `read`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/activity.rs#dashboard_activity](../../rust/crates/core/src/activity.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `view` | no | `["research", "operations", "all"]` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "view": {
      "enum": [
        "research",
        "operations",
        "all"
      ]
    }
  },
  "required": [],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "view": "research"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Typed JSON with explicit coverage and stable source identity.

unknown_item source_unavailable index_unavailable

## dashboard_entity

Read one exact source-qualified browser record with lazy file inventories.

Scope: `read`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/catalog.rs#dashboard_entity](../../rust/crates/core/src/catalog.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `item_id` | yes | `"string"` | `—` |
| `revision_id` | no | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "item_id": {
      "pattern": "^[1-9][0-9]{0,38}$",
      "type": "string"
    },
    "revision_id": {
      "pattern": "^[1-9][0-9]{0,38}$",
      "type": "string"
    }
  },
  "required": [
    "item_id"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "item_id": "2156055679491977121222771780406505881"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Typed JSON with explicit coverage and stable source identity.

unknown_item source_unavailable index_unavailable
