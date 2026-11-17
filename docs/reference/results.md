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
- [metadata_templates](#metadata_templates)
- [get_metadata_template](#get_metadata_template)
- [register_metadata_template](#register_metadata_template)
- [validate_metadata](#validate_metadata)
- [adopt_metadata](#adopt_metadata)
- [organize_dataset](#organize_dataset)
- [link_dataset](#link_dataset)
- [refresh_metadata](#refresh_metadata)
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
- [report_contracts](#report_contracts)
- [report_definitions](#report_definitions)
- [get_report_contract](#get_report_contract)
- [get_report_evidence](#get_report_evidence)
- [get_report_definition](#get_report_definition)
- [register_report_contract](#register_report_contract)
- [register_report_definition](#register_report_definition)
- [validate_report_contract](#validate_report_contract)
- [read_report](#read_report)

## list

Discover native artifacts and imported models/datasets/experiments from the active catalog. For record_type=historical, pass history_ref to results.historical; other entries use results.get. Discovery does not establish file availability. Labels use current presentation curation; results.historical retains the recorded source name.

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



### Result and failures

Artifact list · native registry entries or historical entries with history_ref and detail_action

Shared authentication, schema, scope and handler preconditions still apply.

## get

Read an artifact record by exact ID and revision. Template-backed scientific records include current typed research_summary, robot and metadata_status. Additive operator normalization exposes metadata_provenance with the source digest, evidence and reason; original manifest fields and execution remain preserved.

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



### Result and failures

Registry entity with current display label, immutable manifest fields, registered locations and optional template metadata/provenance

Missing artifact revision or read scope.

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



### Result and failures

Item · id, owner, owner_profile

Unregistered profile or missing artifact.

## metadata_templates

List current immutable output metadata formats; include_revisions exposes previous revisions.

Scope: `read`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/metadata_templates.rs#metadata_template_action](../../rust/crates/core/src/metadata_templates.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `include_revisions` | no | `"boolean"` | `false` |
| `kind` | no | `["dataset", "model", "experiment"]` | `—` |
| `limit` | no | `"integer"` | `—` |
| `offset` | no | `"integer"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "include_revisions": {
      "default": false,
      "type": "boolean"
    },
    "kind": {
      "enum": [
        "dataset",
        "model",
        "experiment"
      ]
    },
    "limit": {
      "maximum": 100,
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
{
  "kind": "dataset"
}
```

Resolve the real template and use its returned sha256; the illustrative digest is a placeholder. Adoption applies only with the preview expected_digest, dry_run=false and a stable key.

### Result and failures

Templates · items, total, next_offset

Unknown or mismatched template, invalid schema or typed metadata, changed source/revision, maintenance fence or insufficient scope.

## get_metadata_template

Read an exact schema, bounded display fields and content digest. Omitting revision explicitly selects the current revision.

Scope: `read`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/metadata_templates.rs#metadata_template_action](../../rust/crates/core/src/metadata_templates.rs).

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
  "id": "loom.dataset",
  "revision": 1
}
```

Resolve the real template and use its returned sha256; the illustrative digest is a placeholder. Adoption applies only with the preview expected_digest, dry_run=false and a stable key.

### Result and failures

Template · id, revision, sha256, output_kind, definition

Unknown or mismatched template, invalid schema or typed metadata, changed source/revision, maintenance fence or insufficient scope.

## register_metadata_template

Register or revise a shared format derived from an exact base. Extensions cannot weaken standard fields or replace their display meaning.

Scope: `submit`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/metadata_templates.rs#metadata_template_action](../../rust/crates/core/src/metadata_templates.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `definition` | yes | `"object"` | `—` |
| `expected_revision` | no | `"integer"` | `—` |
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
      "minimum": 0,
      "type": "integer"
    },
    "id": {
      "maxLength": 96,
      "minLength": 1,
      "type": "string"
    },
    "key": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "reason": {
      "maxLength": 2000,
      "minLength": 1,
      "type": "string"
    }
  },
  "required": [
    "id",
    "definition",
    "reason",
    "key"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "definition": {
    "base_ref": {
      "id": "loom.dataset",
      "revision": 1,
      "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    },
    "fields": [
      {
        "format": "number",
        "id": "friction",
        "label": "Friction",
        "pointer": "/extensions/friction",
        "section": "extensions"
      }
    ],
    "kind": "dataset",
    "label": "Contact demonstrations",
    "schema": {
      "properties": {
        "extensions": {
          "additionalProperties": false,
          "properties": {
            "friction": {
              "minimum": 0,
              "type": "number"
            }
          },
          "type": "object"
        }
      },
      "type": "object"
    }
  },
  "id": "example.dataset",
  "key": "template-example-r1",
  "reason": "Add a source-measured friction field"
}
```

Resolve the real template and use its returned sha256; the illustrative digest is a placeholder. Adoption applies only with the preview expected_digest, dry_run=false and a stable key.

### Result and failures

Registered template · exact revision and hash

Unknown or mismatched template, invalid schema or typed metadata, changed source/revision, maintenance fence or insufficient scope.

## validate_metadata

Validate a draft against an exact template kind, schema and display field types. Structural validation is not scientific acceptance.

Scope: `read`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/metadata_templates.rs#metadata_template_action](../../rust/crates/core/src/metadata_templates.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `kind` | yes | `["dataset", "model", "experiment"]` | `—` |
| `research` | yes | `"object"` | `—` |
| `template_ref` | yes | `"object"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "kind": {
      "enum": [
        "dataset",
        "model",
        "experiment"
      ]
    },
    "research": {
      "type": "object"
    },
    "template_ref": {
      "additionalProperties": false,
      "properties": {
        "id": {
          "maxLength": 96,
          "minLength": 1,
          "type": "string"
        },
        "revision": {
          "minimum": 1,
          "type": "integer"
        },
        "sha256": {
          "pattern": "^[a-f0-9]{64}$",
          "type": "string"
        }
      },
      "required": [
        "id",
        "revision",
        "sha256"
      ],
      "type": "object"
    }
  },
  "required": [
    "kind",
    "template_ref",
    "research"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "kind": "dataset",
  "research": {
    "collection": {
      "status": "unrecorded"
    },
    "robot": {
      "status": "unrecorded"
    }
  },
  "template_ref": {
    "id": "loom.dataset",
    "revision": 1,
    "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
  }
}
```

Resolve the real template and use its returned sha256; the illustrative digest is a placeholder. Adoption applies only with the preview expected_digest, dry_run=false and a stable key.

### Result and failures

Validation · valid, template_ref, summary, scientific_acceptance

Unknown or mismatched template, invalid schema or typed metadata, changed source/revision, maintenance fence or insufficient scope.

## adopt_metadata

Preview and apply an additive operator-declared normalization for an exact existing source. Retain originals and reject changed source or prior annotation. Evidence and the comparable format reference remain provenance; no scientific verification is invented.

Scope: `submit`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/metadata_templates.rs#metadata_template_action](../../rust/crates/core/src/metadata_templates.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `dry_run` | no | `"boolean"` | `true` |
| `evidence` | no | `"array"` | `—` |
| `expected_digest` | no | `"string"` | `—` |
| `format_reference` | no | `"object"` | `—` |
| `key` | no | `"string"` | `—` |
| `reason` | yes | `"string"` | `—` |
| `research` | yes | `"object"` | `—` |
| `target` | yes | `"object"` | `—` |
| `template_ref` | yes | `"object"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "dry_run": {
      "default": true,
      "type": "boolean"
    },
    "evidence": {
      "items": {
        "type": "object"
      },
      "maxItems": 16,
      "type": "array"
    },
    "expected_digest": {
      "pattern": "^[a-f0-9]{64}$",
      "type": "string"
    },
    "format_reference": {
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
    },
    "key": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "reason": {
      "maxLength": 2000,
      "minLength": 1,
      "type": "string"
    },
    "research": {
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
            "entity",
            "historical"
          ]
        }
      },
      "required": [
        "kind",
        "identity"
      ],
      "type": "object"
    },
    "template_ref": {
      "additionalProperties": false,
      "properties": {
        "id": {
          "maxLength": 96,
          "minLength": 1,
          "type": "string"
        },
        "revision": {
          "minimum": 1,
          "type": "integer"
        },
        "sha256": {
          "pattern": "^[a-f0-9]{64}$",
          "type": "string"
        }
      },
      "required": [
        "id",
        "revision",
        "sha256"
      ],
      "type": "object"
    }
  },
  "required": [
    "target",
    "template_ref",
    "research",
    "reason"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "dry_run": true,
  "reason": "Normalize only supported fields from the recorded source",
  "research": {
    "collection": {
      "status": "unrecorded"
    },
    "robot": {
      "status": "unrecorded"
    }
  },
  "target": {
    "identity": {
      "id": "<entity-id>",
      "revision": 1
    },
    "kind": "entity"
  },
  "template_ref": {
    "id": "loom.dataset",
    "revision": 1,
    "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
  }
}
```

Resolve the real template and use its returned sha256; the illustrative digest is a placeholder. Adoption applies only with the preview expected_digest, dry_run=false and a stable key.

### Result and failures

Adoption · target, source_sha256, expected_digest, template_ref, summary, stored, previous_annotation

Unknown or mismatched template, invalid schema or typed metadata, changed source/revision, maintenance fence or insufficient scope.

## organize_dataset

Preview or apply source-bound browsing context under an already linked exact Task and a stable subgroup path. This does not declare data derivation or rewrite source records.

Scope: `submit`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/dataset_organization.rs#organize_dataset](../../rust/crates/core/src/dataset_organization.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `dry_run` | no | `"boolean"` | `true` |
| `expected_digest` | no | `"string"` | `—` |
| `key` | no | `"string"` | `—` |
| `path` | yes | `"array"` | `—` |
| `reason` | yes | `"string"` | `—` |
| `target` | yes | `"object"` | `—` |
| `task_ref` | yes | `"object"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "dry_run": {
      "default": true,
      "type": "boolean"
    },
    "expected_digest": {
      "pattern": "^[a-f0-9]{64}$",
      "type": "string"
    },
    "key": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "path": {
      "items": {
        "additionalProperties": false,
        "properties": {
          "id": {
            "pattern": "^[A-Za-z0-9_.-]{1,64}$",
            "type": "string"
          },
          "label": {
            "maxLength": 80,
            "minLength": 1,
            "type": "string"
          }
        },
        "required": [
          "id",
          "label"
        ],
        "type": "object"
      },
      "maxItems": 12,
      "type": "array"
    },
    "reason": {
      "maxLength": 2000,
      "minLength": 1,
      "type": "string"
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
    },
    "task_ref": {
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
    }
  },
  "required": [
    "target",
    "task_ref",
    "path",
    "reason"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "path": [
    {
      "id": "cohort",
      "label": "Cohort"
    }
  ],
  "reason": "Reuse the evidenced collection context",
  "target": {
    "identity": {
      "id": "<dataset-id>",
      "revision": 1
    },
    "kind": "entity"
  },
  "task_ref": {
    "id": "<linked-task-id>",
    "revision": 1
  }
}
```

Preview first, then apply with expected_digest, a stable key and dry_run=false.

### Result and failures

Organization · dry_run, stored, digest, organization

Exact Dataset or linked Task missing; conflicting subgroup label; source or current assignment changed; maintenance fence.

## link_dataset

Preview and add an exact, acyclic Dataset parent with a typed transformation and registered component evidence. Original records remain immutable; member-level semantic claims remain producer/operator evidence.

Scope: `submit`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/dataset_lineage.rs#link_dataset](../../rust/crates/core/src/dataset_lineage.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `dry_run` | no | `"boolean"` | `true` |
| `evidence` | yes | `"array"` | `—` |
| `expected_digest` | no | `"string"` | `—` |
| `key` | no | `"string"` | `—` |
| `parent` | yes | `"object"` | `—` |
| `primary` | no | `"boolean"` | `false` |
| `reason` | yes | `"string"` | `—` |
| `target` | yes | `"object"` | `—` |
| `transformation` | yes | `"object"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "dry_run": {
      "default": true,
      "type": "boolean"
    },
    "evidence": {
      "items": {
        "additionalProperties": false,
        "properties": {
          "archive_member": {
            "maxLength": 2048,
            "type": "string"
          },
          "component": {
            "maxLength": 2048,
            "minLength": 1,
            "type": "string"
          },
          "member_sha256": {
            "pattern": "^[a-f0-9]{64}$",
            "type": "string"
          },
          "pointer": {
            "maxLength": 2048,
            "type": "string"
          },
          "sha256": {
            "pattern": "^[a-f0-9]{64}$",
            "type": "string"
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
          "target",
          "component",
          "sha256"
        ],
        "type": "object"
      },
      "maxItems": 16,
      "minItems": 1,
      "type": "array"
    },
    "expected_digest": {
      "pattern": "^[a-f0-9]{64}$",
      "type": "string"
    },
    "key": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "parent": {
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
    },
    "primary": {
      "default": false,
      "type": "boolean"
    },
    "reason": {
      "maxLength": 2000,
      "minLength": 1,
      "type": "string"
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
    },
    "transformation": {
      "additionalProperties": false,
      "properties": {
        "config_ref": {
          "type": "object"
        },
        "details": {
          "maxLength": 2000,
          "minLength": 1,
          "type": "string"
        },
        "kind": {
          "enum": [
            "subset",
            "condition_selection",
            "includes",
            "relabel",
            "resample",
            "sensor_rerender",
            "augment",
            "merge"
          ]
        },
        "member_count": {
          "minimum": 0,
          "type": "integer"
        },
        "tool_ref": {
          "type": "object"
        }
      },
      "required": [
        "kind",
        "details"
      ],
      "type": "object"
    }
  },
  "required": [
    "target",
    "parent",
    "transformation",
    "evidence",
    "reason"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "dry_run": true,
  "evidence": [
    {
      "component": "manifest.json",
      "pointer": "/members",
      "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "target": {
        "identity": {
          "id": "source-dataset",
          "revision": 1
        },
        "kind": "entity"
      }
    }
  ],
  "parent": {
    "identity": {
      "id": "source-dataset",
      "revision": 1
    },
    "kind": "entity"
  },
  "reason": "Register verified condition selection",
  "target": {
    "identity": {
      "id": "derived-dataset",
      "revision": 1
    },
    "kind": "entity"
  },
  "transformation": {
    "details": "Selected held-out conditions",
    "kind": "subset",
    "member_count": 20
  }
}
```



### Result and failures

Lineage preview/receipt · digest, dry_run, stored, exact source-bound claim

Missing/non-Dataset revision, unregistered evidence hash, self/cyclic relation, or changed preview digest.

## refresh_metadata

Preview or persist verified scientific browse facts for one exact existing entity/historical revision. The original manifest/body is preserved. Apply requires the preview source digest and a stable request key; unavailable fields are not fabricated.

Scope: `submit`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/research_metadata.rs#refresh_research_metadata](../../rust/crates/core/src/research_metadata.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `dry_run` | no | `"boolean"` | `true` |
| `expected_source_sha256` | no | `"string"` | `—` |
| `key` | no | `"string"` | `—` |
| `reason` | yes | `"string"` | `—` |
| `target` | yes | `"object"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "dry_run": {
      "default": true,
      "type": "boolean"
    },
    "expected_source_sha256": {
      "pattern": "^[a-f0-9]{64}$",
      "type": "string"
    },
    "key": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "reason": {
      "maxLength": 2000,
      "minLength": 1,
      "type": "string"
    },
    "target": {
      "additionalProperties": false,
      "properties": {
        "identity": {
          "type": "object"
        },
        "kind": {
          "enum": [
            "entity",
            "historical"
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
    "target",
    "reason"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "dry_run": true,
  "reason": "Recover registered scientific metadata",
  "target": {
    "identity": {
      "id": "<entity-id>",
      "revision": 1
    },
    "kind": "entity"
  }
}
```

Preview first. Apply with dry_run=false, key and expected_source_sha256 from that preview.

### Result and failures

Metadata receipt · target, source_sha256, facts, inputs, dry_run, stored

Source changed, no recoverable metadata, unsupported target, maintenance fence or missing submit scope.

## scenarios

List versioned research task/protocol contexts. Archived Research Tasks are omitted unless requested.

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



### Result and failures

Research Task summaries with current revision

Shared authentication, schema, scope and handler preconditions still apply.

## get_scenario

Read an exact Research Task revision, or the current revision when omitted. Archive state does not erase accepted context.

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



### Result and failures

Research Task definition, revision and digest

Unknown Scenario or revision.

## scenario_graph

Page explicit links for a Research Task revision; links describe context and do not schedule execution.

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



### Result and failures

Research Task and bounded links

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



### Result and failures

id, new revision and sha256

Stale expected_revision, archived Scenario or key conflict.

## archive_scenario

Remove a Research Task from default discovery using an expected current revision. Exact accepted references remain valid.

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



### Result and failures

Contract · accepted asset adapters, operations, size/count limits and validation scope

Shared authentication, schema, scope and handler preconditions still apply.

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



### Result and failures

Registered metadata page · items, next_cursor, partial, warnings, scope, scan_performed=false

Index unavailable, invalid filter or expired cursor.

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



### Result and failures

Plan · plan_id, state, normalized proposal, validation and optional original receipt

Unknown source contract, conflicting source identity/key or proposal bound exceeded.

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



### Result and failures

Plan · plan_id, state, proposal, created, validation summary, receipt

Plan missing or owned by another actor.

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



### Result and failures

Plan with passed validation summary · file count, bytes and verification time

Plan context/source changed, inaccessible candidate, mismatched digest/size or another actor’s plan.

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



### Result and failures

Applied plan receipt with original operation outcomes; exact replay returns that receipt

Validation required, stale contract/source/grant, changed bytes or conflicting import/binding.

## get_item

Read a compact typed descriptor using a decimal-string Item ID and optional exact revision. Display labels use current curation even when an exact revision is selected; immutable source pins remain unchanged.

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



### Result and failures

Item descriptor · item_id, revision_id, kind, label, native_ref and kind-specific metadata

Invalid Item ID or missing/deleted source revision. File Items also require their canonical inventory and index.

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



### Result and failures

Exact alias · item_id, revision_id, native_ref

The exact native reference has not been adopted, or its source was deleted.

## search_items

Search registered metadata and verified bounded text with exact/prefix/lexical ranking, current/all revisions, origin/relation filters and optional bounded at_least watermark waits. scenario_ref also matches active exact context links, including linked preserved snapshots; context changes expire those cursors. task_id groups every revision of one Task; use scenario_ref for one exact revision and never supply both. A bounded content page can be empty with a non-null next_cursor; follow it to inspect later candidates.

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
| `task_id` | no | `"string"` | `—` |
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
    "task_id": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
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



### Result and failures

Search page · items, next_cursor, partial, consistency, warnings

Index unavailable, invalid/expired cursor, changed source/access or unsupported search.

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
| `task_id` | no | `"string"` | `—` |
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
    "task_id": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
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



### Result and failures

File search page · items, next_cursor, partial, consistency, warnings

Missing owner revision, unavailable index/inventory or expired cursor.

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



### Result and failures

File · item_id, revision_id, parent_item_id, owner_revision_id, path, size, sha256, copies

Unknown File, unavailable index/canonical inventory or changed sealed metadata.

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



### Result and failures

File range · file_item_id, offset, next_offset, total_bytes, data_base64, eof, verification

No granted readable copy, pending verification, changed bytes or invalid offset.

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



### Result and failures

Relation page · items, next_cursor, partial, warnings

Missing source revision or expired cursor; incoming relations require a source-qualified index.

## report_contracts

Page registered immutable reporting templates; default requirements can be adapted with exact evidence before Job submission.

Scope: `read`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/reporting.rs#reporting_action](../../rust/crates/core/src/reporting.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `include_revisions` | no | `"boolean"` | `—` |
| `kind` | no | `["dataset", "model", "experiment"]` | `—` |
| `limit` | no | `"integer"` | `—` |
| `offset` | no | `"integer"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "include_revisions": {
      "type": "boolean"
    },
    "kind": {
      "enum": [
        "dataset",
        "model",
        "experiment"
      ]
    },
    "limit": {
      "maximum": 100,
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



### Result and failures

Exact reporting resource, validated coverage or bounded verified report page

Missing or changed evidence, invalid coverage, schema/population mismatch, conflicting revision/key, denied scope, or unavailable report bytes.

## report_definitions

Page registered metric, factor and evaluation protocol definitions.

Scope: `read`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/reporting.rs#reporting_action](../../rust/crates/core/src/reporting.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `include_revisions` | no | `"boolean"` | `—` |
| `kind` | no | `["metric", "factor", "protocol"]` | `—` |
| `limit` | no | `"integer"` | `—` |
| `offset` | no | `"integer"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "include_revisions": {
      "type": "boolean"
    },
    "kind": {
      "enum": [
        "metric",
        "factor",
        "protocol"
      ]
    },
    "limit": {
      "maximum": 100,
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



### Result and failures

Exact reporting resource, validated coverage or bounded verified report page

Missing or changed evidence, invalid coverage, schema/population mismatch, conflicting revision/key, denied scope, or unavailable report bytes.

## get_report_contract

Read exact coverage, schemas, display descriptors, semantic pins and evidence-backed adaptations.

Scope: `read`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/reporting.rs#reporting_action](../../rust/crates/core/src/reporting.rs).

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
      "maximum": 9223372036854775807,
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
  "id": "loom.report.dataset",
  "revision": 1
}
```



### Result and failures

Exact reporting resource, validated coverage or bounded verified report page

Missing or changed evidence, invalid coverage, schema/population mismatch, conflicting revision/key, denied scope, or unavailable report bytes.

## get_report_evidence

Read a bounded JSON subtree and exact source-body SHA-256 from an immutable artifact, historical record or registered software revision. Use the returned object as adaptation evidence; no database access or digest reconstruction is required.

Scope: `read`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/reporting.rs#reporting_action](../../rust/crates/core/src/reporting.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `pointer` | yes | `"string"` | `—` |
| `target` | yes | `"object"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "pointer": {
      "maxLength": 256,
      "type": "string"
    },
    "target": {
      "additionalProperties": false,
      "properties": {
        "identity": {
          "type": "object"
        },
        "kind": {
          "enum": [
            "entity",
            "historical",
            "component",
            "tool",
            "profile"
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
    "target",
    "pointer"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "pointer": "/metadata/conditioning",
  "target": {
    "identity": {
      "id": "job.policy",
      "revision": 1
    },
    "kind": "entity"
  }
}
```



### Result and failures

Exact target, source SHA-256, JSON pointer and value (maximum 64 KiB)

Missing or inexact source, absent pointer, denied read scope, or evidence subtree larger than 64 KiB.

## get_report_definition

Read an exact metric, factor or protocol definition; semantic meaning and units remain pinned.

Scope: `read`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/reporting.rs#reporting_action](../../rust/crates/core/src/reporting.rs).

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
      "maximum": 9223372036854775807,
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
  "id": "loom.metric.success",
  "revision": 1
}
```



### Result and failures

Exact reporting resource, validated coverage or bounded verified report page

Missing or changed evidence, invalid coverage, schema/population mismatch, conflicting revision/key, denied scope, or unavailable report bytes.

## register_report_contract

Register a new shared analysis contract or immutable revision. A base can be adapted with source-bound evidence; new required analysis needs definitions and reporter support.

Scope: `submit`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/reporting.rs#reporting_action](../../rust/crates/core/src/reporting.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `definition` | yes | `"object"` | `—` |
| `expected_revision` | no | `"integer"` | `—` |
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
      "maximum": 9223372036854775806,
      "minimum": 0,
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
      "maxLength": 2000,
      "minLength": 1,
      "type": "string"
    }
  },
  "required": [
    "id",
    "definition",
    "reason",
    "key"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "definition": {},
  "id": "contact.dataset",
  "key": "contact-v1",
  "reason": "Coverage for the planned study"
}
```



### Result and failures

Exact reporting resource, validated coverage or bounded verified report page

Missing or changed evidence, invalid coverage, schema/population mismatch, conflicting revision/key, denied scope, or unavailable report bytes.

## register_report_definition

Register a metric with units/population/reduction, an SDG factor or a task-specific protocol with an exact primary metric and success predicate.

Scope: `submit`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/reporting.rs#reporting_action](../../rust/crates/core/src/reporting.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `definition` | yes | `"object"` | `—` |
| `expected_revision` | no | `"integer"` | `—` |
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
      "maximum": 9223372036854775806,
      "minimum": 0,
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
      "maxLength": 2000,
      "minLength": 1,
      "type": "string"
    }
  },
  "required": [
    "id",
    "definition",
    "reason",
    "key"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "definition": {},
  "id": "task.success",
  "key": "protocol-v1",
  "reason": "Task success definition"
}
```



### Result and failures

Exact reporting resource, validated coverage or bounded verified report page

Missing or changed evidence, invalid coverage, schema/population mismatch, conflicting revision/key, denied scope, or unavailable report bytes.

## validate_report_contract

Preview normalized effective coverage without registration or execution. Validates schemas, semantic references and adaptation evidence.

Scope: `read`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/reporting.rs#reporting_action](../../rust/crates/core/src/reporting.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `definition` | yes | `"object"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "definition": {
      "type": "object"
    }
  },
  "required": [
    "definition"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "definition": {}
}
```



### Result and failures

Exact reporting resource, validated coverage or bounded verified report page

Missing or changed evidence, invalid coverage, schema/population mismatch, conflicting revision/key, denied scope, or unavailable report bytes.

## read_report

Read a report bound to an exact output revision and requirement. JSON pointers select a field; arrays are paged. Bytes are SHA-256 verified and responses are bounded to 256 KiB.

Scope: `read`. Owner: [results](../features/results.md). Implementation: [rust/crates/core/src/reporting.rs#reporting_action](../../rust/crates/core/src/reporting.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `limit` | no | `"integer"` | `—` |
| `offset` | no | `"integer"` | `—` |
| `pointer` | no | `"string"` | `—` |
| `requirement_id` | yes | `"string"` | `—` |
| `revision` | yes | `"integer"` | `—` |

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
      "maximum": 500,
      "minimum": 1,
      "type": "integer"
    },
    "offset": {
      "maximum": 10000000,
      "minimum": 0,
      "type": "integer"
    },
    "pointer": {
      "maxLength": 256,
      "type": "string"
    },
    "requirement_id": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "revision": {
      "maximum": 9223372036854775807,
      "minimum": 1,
      "type": "integer"
    }
  },
  "required": [
    "id",
    "revision",
    "requirement_id"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "id": "<job>.data",
  "limit": 100,
  "pointer": "/members",
  "requirement_id": "membership",
  "revision": 1
}
```



### Result and failures

Exact reporting resource, validated coverage or bounded verified report page

Missing or changed evidence, invalid coverage, schema/population mismatch, conflicting revision/key, denied scope, or unavailable report bytes.
