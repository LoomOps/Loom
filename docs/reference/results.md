# results

Results and versioned Scenario context

The examples are argument shapes, not live instructions. Replace symbolic identities and hashes with discovered exact records and satisfy the handler preconditions before a mutation. Nested schema constraints are retained below; response notes are descriptive, not a formal response schema.

## Actions

- [list](#list)
- [get](#get)
- [reindex](#reindex)
- [reconnect](#reconnect)
- [annotate](#annotate)
- [history](#history)
- [historical](#historical)
- [assign_owner](#assign_owner)
- [scenarios](#scenarios)
- [get_scenario](#get_scenario)
- [scenario_graph](#scenario_graph)
- [register_scenario](#register_scenario)
- [update_scenario](#update_scenario)
- [archive_scenario](#archive_scenario)
- [link_scenario](#link_scenario)
- [supersede_scenario_link](#supersede_scenario_link)
- [bind_historical_location](#bind_historical_location)
- [reconcile_contract](#reconcile_contract)
- [reconcile_inspect](#reconcile_inspect)
- [reconcile_plan](#reconcile_plan)
- [reconcile_get](#reconcile_get)
- [reconcile_validate](#reconcile_validate)
- [reconcile_apply](#reconcile_apply)
- [get_item](#get_item)
- [resolve_item](#resolve_item)
- [search_items](#search_items)
- [item_files](#item_files)
- [get_file](#get_file)
- [read_file](#read_file)
- [item_relations](#item_relations)

## list

Discover native artifacts and imported models/datasets/experiments from the active catalog. For record_type=historical, pass history_ref to results.historical; other entries use results.get. Discovery does not establish file availability.

Scope: `read`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/results.rs#result_action](../../rust/crates/core/src/results.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `view` | no | `["research", "all", "operations"]` | `"research"` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "view": {
      "default": "research",
      "enum": [
        "research",
        "all",
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
{}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Artifact list · native registry entries or historical entries with history_ref and detail_action

Shared authentication, schema, scope and handler preconditions still apply.

## get

Read an artifact record by ID and revision.

Scope: `read`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/results.rs#result_action](../../rust/crates/core/src/results.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `revision` | yes | `"integer"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "type": "string"
    },
    "revision": {
      "type": "integer"
    }
  },
  "required": [
    "id",
    "revision"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "id": "<entity-id>",
  "revision": 1
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Registry entity view

Artifact access denied or missing revision.

## reindex

Retry publication for a returned execution without restarting GPU execution. Published output identities, hashes, metadata and lineage must remain identical; use reconnect to add a verified location.

Scope: `submit`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/results.rs#result_action](../../rust/crates/core/src/results.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `attempt` | yes | `"string"` | `—` |
| `id` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "attempt": {
      "type": "string"
    },
    "id": {
      "type": "string"
    }
  },
  "required": [
    "id",
    "attempt"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "attempt": "<attempt-id>",
  "id": "<job-id>"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Receipt · id, attempt, state=processing

Not the current Attempt or publication cannot be retried in this state.

## reconnect

Register a verified replacement location for an existing immutable artifact component. Does not rerun execution or change artifact contents.

Scope: `submit`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/results.rs#result_action](../../rust/crates/core/src/results.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `component` | yes | `"string"` | `—` |
| `id` | yes | `"string"` | `—` |
| `ref` | yes | `"string"` | `—` |
| `revision` | yes | `"integer"` | `—` |
| `storage` | no | `"string"` | `"data/0"` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "component": {
      "type": "string"
    },
    "id": {
      "type": "string"
    },
    "ref": {
      "type": "string"
    },
    "revision": {
      "type": "integer"
    },
    "storage": {
      "default": "data/0",
      "description": "Registered output store from system.status.data_stores. Defaults to data/0; frozen per Job.",
      "pattern": "^data/[0-9]+$",
      "type": "string"
    }
  },
  "required": [
    "id",
    "revision",
    "component",
    "ref"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "component": "model.bin",
  "id": "<entity-id>",
  "ref": "restored/model.bin",
  "revision": 1,
  "storage": "data/1"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Location receipt · id, revision, component, ref, status=available

Requires submit scope; unknown store/component, unsafe path or mismatched size/hash is rejected.

## annotate

Append an immutable evidence-backed note to an existing Job; the id is a Job ID, not an artifact ID. Existing results are preserved.

Scope: `submit`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/results.rs#result_action](../../rust/crates/core/src/results.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `annotation_id` | yes | `"string"` | `—` |
| `evidence` | yes | `"object"` | `—` |
| `id` | yes | `"string"` | `—` |
| `reason` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "annotation_id": {
      "type": "string"
    },
    "evidence": {
      "type": "object"
    },
    "id": {
      "type": "string"
    },
    "reason": {
      "type": "string"
    }
  },
  "required": [
    "id",
    "annotation_id",
    "reason",
    "evidence"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "annotation_id": "note-001",
  "evidence": {},
  "id": "<job-id>",
  "reason": "Review note"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Annotation receipt

Requires submit scope.

## history

Read all preserved records with pagination.

Scope: `read`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/results.rs#result_action](../../rust/crates/core/src/results.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `kind` | yes | `"string"` | `—` |
| `limit` | yes | `"integer"` | `—` |
| `offset` | yes | `"integer"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "kind": {
      "type": "string"
    },
    "limit": {
      "type": "integer"
    },
    "offset": {
      "type": "integer"
    }
  },
  "required": [
    "kind",
    "offset",
    "limit"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "kind": "",
  "limit": 50,
  "offset": 0
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

History page

Requires read scope.

## historical

Read immutable history plus current registered-copy availability. Recorded missing does not prove the original was deleted.

Scope: `read`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/results.rs#result_action](../../rust/crates/core/src/results.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `kind` | yes | `"string"` | `—` |
| `revision` | yes | `"string"` | `—` |
| `snapshot` | no | `"string"` | `—` |
| `source` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "type": "string"
    },
    "kind": {
      "type": "string"
    },
    "revision": {
      "type": "string"
    },
    "snapshot": {
      "type": "string"
    },
    "source": {
      "type": "string"
    }
  },
  "required": [
    "source",
    "kind",
    "id",
    "revision"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "id": "<history-id>",
  "kind": "job",
  "revision": "1",
  "source": "loom-ops"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Historical record · body, source identity and current_availability (not_connected, missing, verified, present_unverified, mismatch or unavailable)

The specified record does not exist.

## assign_owner

Set the current responsible user for a model, dataset or artifact.

Scope: `submit`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/results.rs#result_action](../../rust/crates/core/src/results.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `owner` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "type": "string"
    },
    "owner": {
      "type": "string"
    }
  },
  "required": [
    "id",
    "owner"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "id": "<entity-id>",
  "owner": "<profile-id>"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Item · id, owner, owner_profile

Unregistered profile or missing artifact.

## scenarios

List versioned research task/protocol contexts. Archived Scenarios are omitted unless requested.

Scope: `read`. Owner: [scenarios](../features/scenarios.md). Implementation: [rust/crates/core/src/scenario.rs#scenario_query](../../rust/crates/core/src/scenario.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `include_archived` | no | `"boolean"` | `—` |
| `limit` | no | `"integer"` | `—` |
| `offset` | no | `"integer"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "include_archived": {
      "type": "boolean"
    },
    "limit": {
      "maximum": 200,
      "minimum": 1,
      "type": "integer"
    },
    "offset": {
      "minimum": 0,
      "type": "integer"
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

Scenario summaries with current revision

Shared authentication, schema, scope and handler preconditions still apply.

## get_scenario

Read an exact Scenario revision, or the current revision when omitted. Archive state does not erase accepted context.

Scope: `read`. Owner: [scenarios](../features/scenarios.md). Implementation: [rust/crates/core/src/scenario.rs#scenario_query](../../rust/crates/core/src/scenario.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `revision` | no | `"integer"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
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
    "id"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "id": "<scenario-id>",
  "revision": 1
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Scenario definition, revision and digest

Unknown Scenario or revision.

## scenario_graph

Page explicit links for a Scenario revision; links describe context and do not schedule execution.

Scope: `read`. Owner: [scenarios](../features/scenarios.md). Implementation: [rust/crates/core/src/scenario.rs#scenario_query](../../rust/crates/core/src/scenario.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `limit` | no | `"integer"` | `—` |
| `offset` | no | `"integer"` | `—` |
| `revision` | no | `"integer"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "limit": {
      "maximum": 200,
      "minimum": 1,
      "type": "integer"
    },
    "offset": {
      "minimum": 0,
      "type": "integer"
    },
    "revision": {
      "minimum": 1,
      "type": "integer"
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
  "id": "<scenario-id>",
  "limit": 50,
  "offset": 0,
  "revision": 1
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Scenario and bounded links

Unknown Scenario revision.

## register_scenario

Create a versioned task/protocol definition with a nonempty label. Replay the same key and definition after uncertainty.

Scope: `submit`. Owner: [scenarios](../features/scenarios.md). Implementation: [rust/crates/core/src/scenario.rs#create_scenario](../../rust/crates/core/src/scenario.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `definition` | yes | `"object"` | `—` |
| `key` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "definition": {
      "type": "object"
    },
    "key": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    }
  },
  "required": [
    "key",
    "definition"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "definition": {
    "description": "Task and evaluation protocol context.",
    "label": "Example assembly task"
  },
  "key": "scenario-example-v1"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

id, revision=1 and sha256

Invalid definition, changed request key or inactive installation.

## update_scenario

Append a definition revision using expected_revision and a reason. Existing Job/context pins retain their accepted revision.

Scope: `submit`. Owner: [scenarios](../features/scenarios.md). Implementation: [rust/crates/core/src/scenario.rs#update_scenario](../../rust/crates/core/src/scenario.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `definition` | yes | `"object"` | `—` |
| `expected_revision` | yes | `"integer"` | `—` |
| `id` | yes | `"string"` | `—` |
| `key` | yes | `"string"` | `—` |
| `reason` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "definition": {
      "type": "object"
    },
    "expected_revision": {
      "minimum": 1,
      "type": "integer"
    },
    "id": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "key": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "reason": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    }
  },
  "required": [
    "key",
    "id",
    "expected_revision",
    "definition",
    "reason"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "definition": {
    "description": "Revised protocol.",
    "label": "Example assembly task"
  },
  "expected_revision": 1,
  "id": "<scenario-id>",
  "key": "scenario-example-v2",
  "reason": "Protocol clarification"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

id, new revision and sha256

Stale expected_revision, archived Scenario or key conflict.

## archive_scenario

Remove a Scenario from default discovery using an expected current revision. Exact accepted references remain valid.

Scope: `submit`. Owner: [scenarios](../features/scenarios.md). Implementation: [rust/crates/core/src/scenario.rs#archive_scenario](../../rust/crates/core/src/scenario.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `expected_revision` | yes | `"integer"` | `—` |
| `id` | yes | `"string"` | `—` |
| `key` | yes | `"string"` | `—` |
| `reason` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "expected_revision": {
      "minimum": 1,
      "type": "integer"
    },
    "id": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "key": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "reason": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    }
  },
  "required": [
    "key",
    "id",
    "expected_revision",
    "reason"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "expected_revision": 2,
  "id": "<scenario-id>",
  "key": "archive-example",
  "reason": "No longer a current protocol"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

id and archived=true

Stale revision or key conflict.

## link_scenario

Add an explicit typed relationship to an exact existing target with provenance. Does not infer context, duplicate output or grant access.

Scope: `submit`. Owner: [scenarios](../features/scenarios.md). Implementation: [rust/crates/core/src/scenario.rs#link_scenario](../../rust/crates/core/src/scenario.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `key` | yes | `"string"` | `—` |
| `provenance` | yes | `"object"` | `—` |
| `relation` | yes | `["produced_for", "used_by", "demonstrates", "evaluates", "implements", "contextual_reference"]` | `—` |
| `scenario_ref` | yes | `"object"` | `—` |
| `target` | yes | `"object"` | `—` |

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
    "provenance": {
      "type": "object"
    },
    "relation": {
      "enum": [
        "produced_for",
        "used_by",
        "demonstrates",
        "evaluates",
        "implements",
        "contextual_reference"
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
    "target": {
      "additionalProperties": false,
      "properties": {
        "identity": {
          "type": "object"
        },
        "kind": {
          "enum": [
            "job",
            "entity",
            "historical",
            "component",
            "tool",
            "profile",
            "runtime",
            "protocol_artifact"
          ]
        }
      },
      "required": [
        "kind",
        "identity"
      ],
      "type": "object"
    }
  },
  "required": [
    "key",
    "scenario_ref",
    "target",
    "relation",
    "provenance"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "key": "link-example",
  "provenance": {
    "reason": "Explicit requested task"
  },
  "relation": "produced_for",
  "scenario_ref": {
    "id": "<scenario-id>",
    "revision": 1
  },
  "target": {
    "identity": {
      "id": "<job-id>"
    },
    "kind": "job"
  }
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Immutable link receipt

Unknown target/context or conflicting key.

## supersede_scenario_link

Append a supersession receipt for a link, optionally naming an existing replacement. Original link evidence remains.

Scope: `submit`. Owner: [scenarios](../features/scenarios.md). Implementation: [rust/crates/core/src/scenario.rs#supersede_scenario_link](../../rust/crates/core/src/scenario.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `key` | yes | `"string"` | `—` |
| `reason` | yes | `"string"` | `—` |
| `replacement` | no | `["string", "null"]` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "key": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "reason": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "replacement": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "key",
    "id",
    "reason"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "id": "<link-id>",
  "key": "supersede-example",
  "reason": "Corrected explicit context",
  "replacement": null
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Link supersession receipt

Unknown link/replacement or key conflict.

## bind_historical_location

Verify size, SHA-256 and granted store-relative locator, then add current availability to an exact historical file selector. Original history stays unchanged.

Scope: `submit`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/results.rs#result_action](../../rust/crates/core/src/results.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `identity` | yes | `"object"` | `—` |
| `key` | yes | `"string"` | `—` |
| `locator` | yes | `"object"` | `—` |
| `sha256` | yes | `"string"` | `—` |
| `size` | yes | `"integer"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "identity": {
      "type": "object"
    },
    "key": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "locator": {
      "type": "object"
    },
    "sha256": {
      "pattern": "^[a-f0-9]{64}$",
      "type": "string"
    },
    "size": {
      "minimum": 0,
      "type": "integer"
    }
  },
  "required": [
    "key",
    "identity",
    "locator",
    "sha256",
    "size"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "identity": {
    "file": "<file-selector>",
    "id": "<historical-id>",
    "kind": "dataset",
    "revision": "1",
    "snapshot": "<snapshot>",
    "source": "<source>"
  },
  "key": "historical-copy-example",
  "locator": {
    "machine_id": "<machine-id>",
    "ref": "inputs/example.bin",
    "storage_id": "<storage-id>"
  },
  "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
  "size": 0
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Verified additive binding receipt

Unknown historical selector, conflicting original hash, changed bytes or unavailable grant.

## reconcile_contract

Read accepted source adapters, explicit operations, scopes and limits.

Scope: `read`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/reconcile.rs#reconciliation](../../rust/crates/core/src/reconcile.rs).

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

Bounded typed JSON; partial coverage, stable replay receipts and explicit source conflicts.

source_conflict plan_stale validation_required content_changed retention_gap cursor_expired index_unavailable

## reconcile_inspect

Page registered metadata without scanning unregistered payload directories.

Scope: `read`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/reconcile.rs#reconciliation](../../rust/crates/core/src/reconcile.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `cursor` | no | `"string"` | `—` |
| `kinds` | no | `"array"` | `—` |
| `limit` | no | `"integer"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "cursor": {
      "maxLength": 4096,
      "type": "string"
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
    "dataset"
  ],
  "limit": 50
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Bounded typed JSON; partial coverage, stable replay receipts and explicit source conflicts.

source_conflict plan_stale validation_required content_changed retention_gap cursor_expired index_unavailable

## reconcile_plan

Normalize a bounded explicit proposal with a stable key; reject ambiguous or conflicting identities. This does not copy files or create research execution.

Scope: `submit`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/reconcile.rs#reconciliation](../../rust/crates/core/src/reconcile.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `key` | yes | `"string"` | `—` |
| `proposal` | yes | `"object"` | `—` |

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
    "proposal": {
      "type": "object"
    }
  },
  "required": [
    "key",
    "proposal"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "key": "preserved-assets-v1",
  "proposal": {
    "operations": [
      {
        "native_ref": {
          "identity": {
            "id": "existing-id",
            "revision": 1
          },
          "kind": "entity"
        },
        "operation": "adopt"
      }
    ],
    "schema": "loom-reconcile/v1"
  }
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Bounded typed JSON; partial coverage, stable replay receipts and explicit source conflicts.

source_conflict plan_stale validation_required content_changed retention_gap cursor_expired index_unavailable

## reconcile_get

Read the caller’s durable proposal, validation state and original apply receipt.

Scope: `read`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/reconcile.rs#reconciliation](../../rust/crates/core/src/reconcile.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `plan_id` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "plan_id": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    }
  },
  "required": [
    "plan_id"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "plan_id": "plan-id"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Bounded typed JSON; partial coverage, stable replay receipts and explicit source conflicts.

source_conflict plan_stale validation_required content_changed retention_gap cursor_expired index_unavailable

## reconcile_validate

Hash only declared candidates off the execution writer, check grants and original evidence, and record proofs. Validation is structural and byte-integrity only.

Scope: `submit`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/reconcile.rs#reconciliation](../../rust/crates/core/src/reconcile.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `plan_id` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "plan_id": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    }
  },
  "required": [
    "plan_id"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "plan_id": "plan-id"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Bounded typed JSON; partial coverage, stable replay receipts and explicit source conflicts.

source_conflict plan_stale validation_required content_changed retention_gap cursor_expired index_unavailable

## reconcile_apply

Recheck source stamps, exact identities and grants, then atomically add imports or bindings. Historical originals and Job/Attempt counts are preserved.

Scope: `submit`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/reconcile.rs#reconciliation](../../rust/crates/core/src/reconcile.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `plan_id` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "plan_id": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    }
  },
  "required": [
    "plan_id"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "plan_id": "plan-id"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Bounded typed JSON; partial coverage, stable replay receipts and explicit source conflicts.

source_conflict plan_stale validation_required content_changed retention_gap cursor_expired index_unavailable

## get_item

Read a compact typed descriptor using a decimal-string Item ID and optional exact revision.

Scope: `read`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/items.rs#get_item](../../rust/crates/core/src/items.rs).

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
  "item_id": "12345678901234567890"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Bounded typed JSON with exact IDs; explicit errors or partial coverage when unavailable.

unknown_item index_unavailable cursor_expired source_unavailable content_changed query_too_broad

## resolve_item

Resolve an already adopted full native identity without allocation, promotion or latest-snapshot substitution.

Scope: `read`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/items.rs#resolve_item](../../rust/crates/core/src/items.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `native_ref` | yes | `"object"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "native_ref": {
      "additionalProperties": false,
      "properties": {
        "identity": {
          "type": "object"
        },
        "kind": {
          "type": "string"
        }
      },
      "required": [
        "kind",
        "identity"
      ],
      "type": "object"
    }
  },
  "required": [
    "native_ref"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "native_ref": {
    "identity": {
      "id": "<job-id>"
    },
    "kind": "job"
  }
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Bounded typed JSON with exact IDs; explicit errors or partial coverage when unavailable.

unknown_item index_unavailable cursor_expired source_unavailable content_changed query_too_broad

## search_items

Search registered metadata and verified bounded text with exact/prefix/lexical ranking, current/all revisions, origin/relation filters and optional bounded at_least watermark waits.

Scope: `read`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/index_store.rs#search_items](../../rust/crates/core/src/index_store.rs).

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
{}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Bounded typed JSON with exact IDs; explicit errors or partial coverage when unavailable.

unknown_item index_unavailable cursor_expired source_unavailable content_changed query_too_broad

## item_files

Page the sealed registered file inventory for an exact Item revision; File IDs survive derived index rebuilds.

Scope: `read`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/store.rs#execute](../../rust/crates/core/src/store.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `at_least` | no | `"string"` | `—` |
| `availability` | no | `["verified", "source_unavailable", "recorded"]` | `—` |
| `category` | no | `"string"` | `—` |
| `cursor` | no | `"string"` | `—` |
| `dependency_ref` | no | `"object"` | `—` |
| `history` | no | `["current", "all"]` | `—` |
| `item_id` | yes | `"string"` | `—` |
| `kinds` | no | `"array"` | `—` |
| `limit` | no | `"integer"` | `—` |
| `native_kind` | no | `"string"` | `—` |
| `origin_type` | no | `"string"` | `—` |
| `parent_item_id` | no | `"string"` | `—` |
| `recorded_from` | no | `"number"` | `—` |
| `recorded_to` | no | `"number"` | `—` |
| `related_to` | no | `"object"` | `—` |
| `revision_id` | no | `"string"` | `—` |
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
    "item_id": {
      "pattern": "^[1-9][0-9]{0,38}$",
      "type": "string"
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
    "revision_id": {
      "pattern": "^[1-9][0-9]{0,38}$",
      "type": "string"
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
  "item_id": "12345678901234567890",
  "limit": 50
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Bounded typed JSON with exact IDs; explicit errors or partial coverage when unavailable.

unknown_item index_unavailable cursor_expired source_unavailable content_changed query_too_broad

## get_file

Read recorded digest, exact owner revision and currently registered candidate copies.

Scope: `read`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/inventory.rs#get_file](../../rust/crates/core/src/inventory.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `file_item_id` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "file_item_id": {
      "pattern": "^[1-9][0-9]{0,38}$",
      "type": "string"
    }
  },
  "required": [
    "file_item_id"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "file_item_id": "12345678901234567890"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Bounded typed JSON with exact IDs; explicit errors or partial coverage when unavailable.

unknown_item index_unavailable cursor_expired source_unavailable content_changed query_too_broad

## read_file

Read at most 262144 verified bytes; offsets are decimal strings and returned data_base64 preserves raw bytes.

Scope: `read`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/inventory.rs#read_file](../../rust/crates/core/src/inventory.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `file_item_id` | yes | `"string"` | `—` |
| `limit` | no | `"integer"` | `—` |
| `offset` | no | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "file_item_id": {
      "pattern": "^[1-9][0-9]{0,38}$",
      "type": "string"
    },
    "limit": {
      "maximum": 262144,
      "minimum": 1,
      "type": "integer"
    },
    "offset": {
      "pattern": "^[0-9]{1,20}$",
      "type": "string"
    }
  },
  "required": [
    "file_item_id"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "file_item_id": "12345678901234567890",
  "limit": 4096,
  "offset": "0"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Bounded typed JSON with exact IDs; explicit errors or partial coverage when unavailable.

unknown_item index_unavailable cursor_expired source_unavailable content_changed query_too_broad

## item_relations

Page exact outgoing declarations and indexed reverse software use. Cursors pin source revision, authority head, actor and index generation; unresolved references remain explicit.

Scope: `read`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/graph.rs#item_graph](../../rust/crates/core/src/graph.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `cursor` | no | `"string"` | `—` |
| `direction` | no | `["outgoing", "both"]` | `—` |
| `include_support` | no | `"boolean"` | `—` |
| `item_id` | yes | `"string"` | `—` |
| `limit` | no | `"integer"` | `—` |
| `revision_id` | no | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "cursor": {
      "maxLength": 4096,
      "type": "string"
    },
    "direction": {
      "enum": [
        "outgoing",
        "both"
      ]
    },
    "include_support": {
      "type": "boolean"
    },
    "item_id": {
      "pattern": "^[1-9][0-9]{0,38}$",
      "type": "string"
    },
    "limit": {
      "maximum": 200,
      "minimum": 1,
      "type": "integer"
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
  "item_id": "12345678901234567890",
  "limit": 50
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Bounded typed JSON with exact IDs; explicit errors or partial coverage when unavailable.

unknown_item index_unavailable cursor_expired source_unavailable content_changed query_too_broad
