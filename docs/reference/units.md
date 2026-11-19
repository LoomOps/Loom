# units

Independent execution and resource control

The examples are argument shapes, not live instructions. Replace symbolic identities and hashes with discovered exact records and satisfy the handler preconditions before a mutation. Nested schema constraints are retained below; response notes are descriptive, not a formal response schema.

## Actions

- [submit](#submit)
- [get](#get)
- [hold](#hold)
- [resume](#resume)
- [cancel](#cancel)
- [reorder](#reorder)

## submit

Submit one standalone Unit. Pin scenario_ref for task-specific work. The Task pin is required when the Profile declares Dataset, Policy or Experiment outputs, which publish as Reference. General infrastructure Units may publish Artifacts without a Task. Scientific Policy outputs require the exact Dataset and Experiment outputs require the exact evaluation Dataset and Policy under the same pinned Task.

Scope: `submit`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/jobs.rs#submit_execution](../../rust/crates/core/src/jobs.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `category` | no | `["sdg", "data_preparation", "fine_tuning", "evaluation", "pipeline", "development", "other"]` | `"other"` |
| `depends_on` | no | `"array"` | `—` |
| `execution` | no | `"object"` | `—` |
| `execution_mode` | no | `["full", "pilot", "smoke", null]` | `null` |
| `gpu_access` | no | `"object"` | `—` |
| `key` | yes | `"string"` | `—` |
| `kind` | yes | `"string"` | `—` |
| `notify` | yes | `"string"` | `—` |
| `output_storage` | no | `"string"` | `"data/0"` |
| `owner` | yes | `"string"` | `—` |
| `params` | yes | `"object"` | `—` |
| `pilot` | no | `"object"` | `—` |
| `presentation` | no | `"object"` | `—` |
| `queue_position` | no | `["tail", "next"]` | `"tail"` |
| `scenario_ref` | no | `"object"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "category": {
      "default": "other",
      "description": "Job purpose; does not select an execution runner.",
      "enum": [
        "sdg",
        "data_preparation",
        "fine_tuning",
        "evaluation",
        "pipeline",
        "development",
        "other"
      ],
      "type": "string"
    },
    "depends_on": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "execution": {
      "additionalProperties": false,
      "properties": {
        "max_attempts": {
          "minimum": 1,
          "type": "integer"
        },
        "retry_backoff_seconds": {
          "maximum": 86400,
          "minimum": 0,
          "type": "integer"
        },
        "retry_on": {
          "items": {
            "enum": [
              "exit_nonzero",
              "timeout",
              "launch_failed",
              "interrupted"
            ],
            "type": "string"
          },
          "type": "array",
          "uniqueItems": true
        },
        "timeout_seconds": {
          "minimum": 1,
          "type": "integer"
        }
      },
      "type": "object"
    },
    "execution_mode": {
      "default": null,
      "description": "Full run, reduced pilot, or short smoke check. Null means unrecorded.",
      "enum": [
        "full",
        "pilot",
        "smoke",
        null
      ],
      "type": [
        "string",
        "null"
      ]
    },
    "gpu_access": {
      "additionalProperties": false,
      "properties": {
        "devices": {
          "items": {
            "maxLength": 2048,
            "minLength": 1,
            "type": "string"
          },
          "maxItems": 8,
          "type": "array",
          "uniqueItems": true
        },
        "memory_mib": {
          "minimum": 1,
          "type": "integer"
        },
        "mode": {
          "enum": [
            "shared",
            "exclusive"
          ]
        }
      },
      "required": [
        "mode"
      ],
      "type": "object"
    },
    "key": {
      "type": "string"
    },
    "kind": {
      "type": "string"
    },
    "notify": {
      "type": "string"
    },
    "output_storage": {
      "default": "data/0",
      "description": "Registered output store from system.status.data_stores. Defaults to data/0; frozen per Job.",
      "pattern": "^data/[0-9]+$",
      "type": "string"
    },
    "owner": {
      "type": "string"
    },
    "params": {
      "type": "object"
    },
    "pilot": {
      "additionalProperties": false,
      "properties": {
        "preflight_for": {
          "maxLength": 2048,
          "minLength": 1,
          "type": "string"
        },
        "reduced_scope": {
          "maxLength": 2048,
          "minLength": 1,
          "type": "string"
        }
      },
      "required": [
        "preflight_for",
        "reduced_scope"
      ],
      "type": "object"
    },
    "presentation": {
      "additionalProperties": false,
      "properties": {
        "experiment": {
          "maxLength": 256,
          "minLength": 1,
          "type": "string"
        },
        "label": {
          "maxLength": 200,
          "minLength": 1,
          "type": "string"
        },
        "output_roles": {
          "additionalProperties": {
            "enum": [
              "main",
              "support",
              "test"
            ]
          },
          "maxProperties": 256,
          "type": "object"
        },
        "parent_job": {
          "maxLength": 256,
          "minLength": 1,
          "type": "string"
        },
        "request_ref": {
          "maxLength": 1024,
          "minLength": 1,
          "type": "string"
        },
        "role": {
          "enum": [
            "main",
            "support",
            "test"
          ]
        },
        "scope": {
          "enum": [
            "research",
            "operations"
          ]
        }
      },
      "type": "object"
    },
    "queue_position": {
      "default": "tail",
      "enum": [
        "tail",
        "next"
      ],
      "type": "string"
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
    }
  },
  "required": [
    "key",
    "kind",
    "owner",
    "notify",
    "params"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "execution_mode": "smoke",
  "key": "quick-check",
  "kind": "cpu.smoke",
  "notify": "owner",
  "owner": "owner",
  "params": {
    "fail": false,
    "seconds": 0
  }
}
```



### Result and failures

Accepted identity and execution details

Invalid inputs, pinned software, resource request or insufficient scope.

## get

Apply get to an exact Unit ID.

Scope: `read`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/query.rs#inspect_job](../../rust/crates/core/src/query.rs).

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
  "id": "<unit-id>"
}
```



### Result and failures

Returns current state, Stage and Attempt receipts, optional Job membership, output and scheduling evidence.

The ID does not exist.

## hold

Apply hold to an exact Unit ID.

Scope: `submit`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/jobs.rs#job_action](../../rust/crates/core/src/jobs.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `reason` | no | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "type": "string"
    },
    "reason": {
      "maxLength": 1000,
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
  "id": "<unit-id>"
}
```



### Result and failures

Updated shared queue with physical entries and logical entries

Permission denied. A physical Job with no waiting or preparing stage returns a queue conflict (409).

## resume

Apply resume to an exact Unit ID.

Scope: `submit`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/jobs.rs#job_action](../../rust/crates/core/src/jobs.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `reason` | no | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "type": "string"
    },
    "reason": {
      "maxLength": 1000,
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
  "id": "<unit-id>"
}
```



### Result and failures

Updated shared queue with physical entries and logical entries

Permission denied. Releasing a failed Job's hold does not rerun its failed execution.

## cancel

Apply cancel to an exact Unit ID.

Scope: `cancel`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/jobs.rs#job_action](../../rust/crates/core/src/jobs.rs).

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
  "id": "<unit-id>"
}
```



### Result and failures

Cancellation receipt · id, state

Missing cancel scope.

## reorder

Apply reorder to an exact Unit ID.

Scope: `submit`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/jobs.rs#job_action](../../rust/crates/core/src/jobs.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `position` | yes | `["next", "tail", "before", "after"]` | `—` |
| `reason` | no | `"string"` | `—` |
| `reference` | no | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "type": "string"
    },
    "position": {
      "enum": [
        "next",
        "tail",
        "before",
        "after"
      ],
      "type": "string"
    },
    "reason": {
      "maxLength": 1000,
      "type": "string"
    },
    "reference": {
      "type": "string"
    }
  },
  "required": [
    "id",
    "position"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "id": "<unit-id>",
  "position": "next"
}
```



### Result and failures

Updated shared queue with physical entries and logical entries

Permission denied, invalid position or reference no longer waiting. A target with no waiting execution returns a queue conflict (409).
