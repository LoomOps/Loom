# jobs

Research Job admission and history

The examples are argument shapes, not live instructions. Replace symbolic identities and hashes with discovered exact records and satisfy the handler preconditions before a mutation. Nested schema constraints are retained below; response notes are descriptive, not a formal response schema.

## Actions

- [list](#list)
- [get](#get)
- [submit](#submit)
- [submit_workflow](#submit_workflow)
- [curate](#curate)
- [cancel](#cancel)
- [logs](#logs)
- [queue](#queue)
- [reorder](#reorder)
- [hold](#hold)
- [resume](#resume)
- [assign_owner](#assign_owner)
- [create](#create)
- [revise_plan](#revise_plan)
- [submit_research](#submit_research)
- [revise_research](#revise_research)
- [preview_outcome](#preview_outcome)
- [preview_history](#preview_history)
- [prune_logs](#prune_logs)
- [log_streams](#log_streams)
- [search_logs](#search_logs)
- [read_log](#read_log)

## list

List scheduled Jobs, including each execution previously submitted in a workflow. Populated group receipts remain available by exact ID. Unbound reservations remain visible.

Scope: `read`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/query.rs#job_list](../../rust/crates/core/src/query.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `category` | no | `["sdg", "data_preparation", "fine_tuning", "evaluation", "pipeline", "development", "other"]` | `"other"` |
| `execution_mode` | no | `["full", "pilot", "smoke"]` | `—` |
| `include_consolidated` | no | `"boolean"` | `false` |
| `limit` | no | `"integer"` | `250` |
| `offset` | no | `"integer"` | `0` |
| `view` | no | `["research", "all", "operations"]` | `"all"` |

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
    "execution_mode": {
      "enum": [
        "full",
        "pilot",
        "smoke"
      ],
      "type": "string"
    },
    "include_consolidated": {
      "default": false,
      "type": "boolean"
    },
    "limit": {
      "default": 250,
      "maximum": 1000,
      "minimum": 1,
      "type": "integer"
    },
    "offset": {
      "default": 0,
      "minimum": 0,
      "type": "integer"
    },
    "view": {
      "default": "all",
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
{
  "limit": 25,
  "offset": 0
}
```



### Result and failures

Job[] · id, status, state, owner, attempt, recovery, category, execution_mode

Shared authentication, schema, scope and handler preconditions still apply.

## get

Read an accepted research Job, its plan, member Units, status and deliverable receipts. Use units.get for a member Unit's Attempts, stages and recovery. Exact legacy reads retain original workflow membership, Attempts, consolidation and timing evidence. The optional family view includes related work and recent activity. Use unit.job_id or recorded execution membership to determine scheduling ownership.

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
  "id": "<job-id>"
}
```



### Result and failures

Job status, accepted plan, Units and result, or exact legacy execution details

The ID does not exist.

## submit

Queue one physical execution using the compatibility contract. New research outcomes use jobs.submit_research. Independent supporting work uses units.submit. Profiles declaring Dataset, Policy or Experiment outputs require scenario_ref. A legacy outcome can pin direct deliverables and scientific reporting. Pilot requires pilot.preflight_for and pilot.reduced_scope. Scientific Policy outputs require the exact Dataset and Experiment outputs require the exact evaluation Dataset and Policy under the same pinned Task.

Scope: `submit`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/jobs.rs#submit](../../rust/crates/core/src/jobs.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `category` | no | `["sdg", "data_preparation", "fine_tuning", "evaluation", "pipeline", "development", "other"]` | `"other"` |
| `consolidates` | no | `"array"` | `—` |
| `depends_on` | no | `"array"` | `—` |
| `execution` | no | `"object"` | `—` |
| `execution_mode` | no | `["full", "pilot", "smoke", null]` | `null` |
| `key` | yes | `"string"` | `—` |
| `kind` | yes | `"string"` | `—` |
| `notify` | yes | `"string"` | `—` |
| `outcome` | no | `"object"` | `—` |
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
    "consolidates": {
      "description": "Explicitly consolidate completed producer Jobs into this logical Job. Original execution/history and outputs remain intact.",
      "items": {
        "pattern": "^[a-f0-9]{32}$",
        "type": "string"
      },
      "maxItems": 64,
      "minItems": 1,
      "type": "array",
      "uniqueItems": true
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
    "key": {
      "type": "string"
    },
    "kind": {
      "type": "string"
    },
    "notify": {
      "type": "string"
    },
    "outcome": {
      "additionalProperties": false,
      "properties": {
        "deliverables": {
          "items": {
            "additionalProperties": false,
            "properties": {
              "alias": {
                "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                "type": "string"
              },
              "entity": {
                "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                "type": "string"
              },
              "kind": {
                "enum": [
                  "artifact",
                  "dataset",
                  "model",
                  "experiment"
                ]
              }
            },
            "required": [
              "alias",
              "entity",
              "kind"
            ],
            "type": "object"
          },
          "maxItems": 256,
          "type": "array"
        },
        "goal": {
          "maxLength": 200,
          "minLength": 1,
          "type": "string"
        },
        "science": {
          "additionalProperties": false,
          "properties": {
            "deliverables": {
              "additionalProperties": {
                "additionalProperties": false,
                "properties": {
                  "activity": {
                    "maxLength": 2048,
                    "minLength": 1,
                    "type": "string"
                  },
                  "contract_ref": {
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
                  },
                  "inputs": {
                    "additionalProperties": {
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
                    "maxProperties": 64,
                    "type": "object"
                  },
                  "sessions": {
                    "items": {
                      "additionalProperties": false,
                      "properties": {
                        "configuration": {
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
                        },
                        "domain": {
                          "enum": [
                            "simulation",
                            "real"
                          ]
                        },
                        "id": {
                          "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                          "type": "string"
                        }
                      },
                      "required": [
                        "id",
                        "domain",
                        "configuration"
                      ],
                      "type": "object"
                    },
                    "maxItems": 64,
                    "type": "array"
                  },
                  "template_ref": {
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
                  "activity",
                  "template_ref",
                  "contract_ref"
                ],
                "type": "object"
              },
              "maxProperties": 64,
              "minProperties": 1,
              "type": "object"
            },
            "hypothesis": {
              "additionalProperties": false,
              "properties": {
                "aliases": {
                  "items": {
                    "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                    "type": "string"
                  },
                  "maxItems": 64,
                  "minItems": 1,
                  "type": "array",
                  "uniqueItems": true
                },
                "analysis": {
                  "maxLength": 2048,
                  "minLength": 1,
                  "type": "string"
                },
                "metric": {
                  "maxLength": 2048,
                  "minLength": 1,
                  "type": "string"
                },
                "population": {
                  "maxLength": 2048,
                  "minLength": 1,
                  "type": "string"
                },
                "statement": {
                  "maxLength": 2048,
                  "minLength": 1,
                  "type": "string"
                }
              },
              "required": [
                "statement",
                "analysis",
                "population",
                "metric",
                "aliases"
              ],
              "type": "object"
            }
          },
          "required": [
            "deliverables"
          ],
          "type": "object"
        }
      },
      "required": [
        "goal"
      ],
      "type": "object"
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
  "category": "fine_tuning",
  "execution_mode": "smoke",
  "key": "example-run-001",
  "kind": "<registered-profile-id>",
  "notify": "<agent-id>",
  "owner": "<owner-id>",
  "params": {}
}
```



### Result and failures

Submission · id

Unregistered profile, parameter schema mismatch, denied input access or stopped admission. Reusing a key with different execution content causes a conflict.

## submit_workflow

Replay the original receipt for an existing key. New graph submissions return separate_jobs (409). Use jobs.submit_research for one research outcome with its Units and link separate Jobs through exact outputs.

Scope: `submit`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/jobs.rs#submit_workflow](../../rust/crates/core/src/jobs.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `category` | no | `["sdg", "data_preparation", "fine_tuning", "evaluation", "pipeline", "development", "other"]` | `—` |
| `key` | yes | `"string"` | `—` |
| `label` | yes | `"string"` | `—` |
| `nodes` | yes | `"array"` | `—` |
| `notify` | yes | `"string"` | `—` |
| `outputs` | no | `"object"` | `—` |
| `owner` | yes | `"string"` | `—` |
| `presentation` | no | `"object"` | `—` |
| `scenario_ref` | no | `"object"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "category": {
      "enum": [
        "sdg",
        "data_preparation",
        "fine_tuning",
        "evaluation",
        "pipeline",
        "development",
        "other"
      ]
    },
    "key": {
      "maxLength": 128,
      "minLength": 1,
      "type": "string"
    },
    "label": {
      "maxLength": 200,
      "minLength": 1,
      "type": "string"
    },
    "nodes": {
      "items": {
        "additionalProperties": false,
        "properties": {
          "depends_on": {
            "items": {
              "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
              "type": "string"
            },
            "maxItems": 64,
            "type": "array",
            "uniqueItems": true
          },
          "id": {
            "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
            "type": "string"
          },
          "label": {
            "maxLength": 200,
            "minLength": 1,
            "type": "string"
          },
          "spec": {
            "type": "object"
          }
        },
        "required": [
          "id",
          "label",
          "spec"
        ],
        "type": "object"
      },
      "maxItems": 64,
      "minItems": 1,
      "type": "array"
    },
    "notify": {
      "type": "string"
    },
    "outputs": {
      "additionalProperties": {
        "additionalProperties": false,
        "properties": {
          "entity": {
            "type": "string"
          },
          "node": {
            "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
            "type": "string"
          }
        },
        "required": [
          "node",
          "entity"
        ],
        "type": "object"
      },
      "maxProperties": 256,
      "type": "object"
    },
    "owner": {
      "type": "string"
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
    "label",
    "owner",
    "notify",
    "nodes"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "key": "example-work",
  "label": "Train and evaluate",
  "nodes": [
    {
      "id": "check",
      "label": "Check",
      "spec": {
        "kind": "cpu.smoke",
        "params": {
          "fail": false,
          "seconds": 0
        }
      }
    }
  ],
  "notify": "<user>",
  "owner": "<user>"
}
```



### Result and failures

Logical Job ID and node execution IDs

409 separate_jobs for a new request; 409 idempotency_conflict when the original key is reused with a different request.

## curate

Preview or apply additive, evidence-backed presentation bindings. Original execution, timestamps, results and software revisions remain intact. Omit dry_run for a preview; false applies the exact batch once. User-requested Runtime preparation and diagnostics remain user work; test/System is reserved for platform checks. parent_job groups the same requested outcome across follow-ups without changing scheduler dependencies or Runtime pins.

Scope: `submit`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/query.rs#curate_research](../../rust/crates/core/src/query.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `changes` | yes | `"array"` | `—` |
| `dry_run` | no | `"boolean"` | `true` |
| `key` | yes | `"string"` | `—` |
| `reason` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "changes": {
      "items": {
        "additionalProperties": false,
        "properties": {
          "id": {
            "maxLength": 512,
            "minLength": 1,
            "type": "string"
          },
          "kind": {
            "enum": [
              "job",
              "entity",
              "component",
              "tool",
              "profile",
              "runtime"
            ]
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
                "type": [
                  "string",
                  "null"
                ]
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
          }
        },
        "required": [
          "kind",
          "id",
          "presentation"
        ],
        "type": "object"
      },
      "maxItems": 1000,
      "minItems": 1,
      "type": "array"
    },
    "dry_run": {
      "default": true,
      "type": "boolean"
    },
    "key": {
      "maxLength": 128,
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
    "key",
    "reason",
    "changes"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "changes": [
    {
      "id": "<job>",
      "kind": "job",
      "presentation": {
        "role": "test",
        "scope": "operations"
      }
    }
  ],
  "key": "review-1",
  "reason": "Reviewed request and execution evidence"
}
```



### Result and failures

Preview or idempotent batch receipt

Unknown identity, repeated item, parent cycle, conflicting batch key, maintenance or scope denied.

## cancel

Cancel the specified Job, regardless of owner. For active execution, verify actual resource return separately.

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
  "id": "<job-id>"
}
```



### Result and failures

Cancellation receipt · id, state

Missing cancel scope.

## logs

Read Attempt logs using a byte cursor. Continue from next_offset.

Scope: `read`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/query.rs#job_logs](../../rust/crates/core/src/query.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `attempt` | yes | `"string"` | `—` |
| `id` | yes | `"string"` | `—` |
| `limit` | yes | `"integer"` | `—` |
| `offset` | yes | `"integer"` | `—` |

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
    },
    "limit": {
      "maximum": 65536,
      "minimum": 1,
      "type": "integer"
    },
    "offset": {
      "minimum": 0,
      "type": "integer"
    }
  },
  "required": [
    "id",
    "attempt",
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
  "attempt": "<attempt-id>",
  "id": "<job-id>",
  "limit": 65536,
  "offset": 0
}
```



### Result and failures

Log range · available, offset, next_offset, text

Attempt belongs to another Job, negative offset or limit outside 1–65536.

## queue

Read every user's waiting order and available controls.

Scope: `read`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/query.rs#queue](../../rust/crates/core/src/query.rs).

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

Queue · entries with position, held, reason, capabilities and optional producer progress forecast for a running Unit

Shared authentication, schema, scope and handler preconditions still apply.

## reorder

Move a waiting Job; a logical workflow moves its remaining waiting nodes as one group. Preserve running Jobs and dependencies.

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
  "id": "<job-id>",
  "position": "next"
}
```



### Result and failures

Updated shared queue with physical entries and logical entries

Permission denied, invalid position or reference no longer waiting. A target with no waiting execution returns a queue conflict (409).

## hold

Prevent new stages of a waiting Job. Active preparation finishes, but run cannot start.

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
  "id": "<job-id>"
}
```



### Result and failures

Updated shared queue with physical entries and logical entries

Permission denied. A physical Job with no waiting or preparing stage returns a queue conflict (409).

## resume

Release an ordinary hold while preserving order. This is not error recovery or checkpoint restart.

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
  "id": "<job-id>"
}
```



### Result and failures

Updated shared queue with physical entries and logical entries

Permission denied. Releasing a failed Job's hold does not rerun its failed execution.

## assign_owner

Set the current responsible user, preserving original execution and submitter records.

Scope: `submit`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/query.rs#assign_job_owner](../../rust/crates/core/src/query.rs).

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
  "id": "<job-id>",
  "owner": "<profile-id>"
}
```



### Result and failures

Item · id, owner, owner_profile

Unregistered profile or missing Job.

## create

Reserve one execution or verify Runtime receipts. Use plan.run for a pending or bound execution and name its output entities. Catalog deliverables and Profiles that publish them require scenario_ref. Use jobs.submit_research with an unbound Unit to reserve a new research outcome.

Scope: `submit`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/plans.rs#write_plan](../../rust/crates/core/src/plans.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `category` | no | `["sdg", "data_preparation", "fine_tuning", "evaluation", "pipeline", "development", "other"]` | `—` |
| `key` | yes | `"string"` | `—` |
| `notify` | yes | `"string"` | `—` |
| `outcome` | yes | `"object"` | `—` |
| `owner` | yes | `"string"` | `—` |
| `plan` | yes | `"see schema"` | `—` |
| `presentation` | no | `"object"` | `—` |
| `scenario_ref` | no | `"object"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "category": {
      "enum": [
        "sdg",
        "data_preparation",
        "fine_tuning",
        "evaluation",
        "pipeline",
        "development",
        "other"
      ]
    },
    "key": {
      "maxLength": 128,
      "minLength": 1,
      "type": "string"
    },
    "notify": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "outcome": {
      "additionalProperties": false,
      "properties": {
        "deliverables": {
          "items": {
            "oneOf": [
              {
                "additionalProperties": false,
                "properties": {
                  "alias": {
                    "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                    "type": "string"
                  },
                  "from": {
                    "additionalProperties": false,
                    "properties": {
                      "entity": {
                        "type": "string"
                      },
                      "node": {
                        "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                        "type": "string"
                      }
                    },
                    "required": [
                      "node",
                      "entity"
                    ],
                    "type": "object"
                  },
                  "kind": {
                    "enum": [
                      "artifact",
                      "dataset",
                      "model",
                      "experiment"
                    ]
                  }
                },
                "required": [
                  "alias",
                  "kind",
                  "from"
                ],
                "type": "object"
              },
              {
                "additionalProperties": false,
                "properties": {
                  "alias": {
                    "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                    "type": "string"
                  },
                  "kind": {
                    "const": "runtime_receipt"
                  },
                  "runtime": {
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
                      },
                      "sha256": {
                        "pattern": "^[a-f0-9]{64}$",
                        "type": "string"
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
                  "alias",
                  "kind",
                  "runtime"
                ],
                "type": "object"
              },
              {
                "additionalProperties": false,
                "properties": {
                  "alias": {
                    "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                    "type": "string"
                  },
                  "entity": {
                    "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                    "type": "string"
                  },
                  "kind": {
                    "enum": [
                      "artifact",
                      "dataset",
                      "model",
                      "experiment"
                    ]
                  }
                },
                "required": [
                  "alias",
                  "entity",
                  "kind"
                ],
                "type": "object"
              }
            ]
          },
          "maxItems": 256,
          "type": "array"
        },
        "goal": {
          "maxLength": 200,
          "minLength": 1,
          "type": "string"
        },
        "science": {
          "additionalProperties": false,
          "properties": {
            "deliverables": {
              "additionalProperties": {
                "additionalProperties": false,
                "properties": {
                  "activity": {
                    "maxLength": 2048,
                    "minLength": 1,
                    "type": "string"
                  },
                  "contract_ref": {
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
                  },
                  "inputs": {
                    "additionalProperties": {
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
                    "maxProperties": 64,
                    "type": "object"
                  },
                  "sessions": {
                    "items": {
                      "additionalProperties": false,
                      "properties": {
                        "configuration": {
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
                        },
                        "domain": {
                          "enum": [
                            "simulation",
                            "real"
                          ]
                        },
                        "id": {
                          "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                          "type": "string"
                        }
                      },
                      "required": [
                        "id",
                        "domain",
                        "configuration"
                      ],
                      "type": "object"
                    },
                    "maxItems": 64,
                    "type": "array"
                  },
                  "template_ref": {
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
                  "activity",
                  "template_ref",
                  "contract_ref"
                ],
                "type": "object"
              },
              "maxProperties": 64,
              "minProperties": 1,
              "type": "object"
            },
            "hypothesis": {
              "additionalProperties": false,
              "properties": {
                "aliases": {
                  "items": {
                    "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                    "type": "string"
                  },
                  "maxItems": 64,
                  "minItems": 1,
                  "type": "array",
                  "uniqueItems": true
                },
                "analysis": {
                  "maxLength": 2048,
                  "minLength": 1,
                  "type": "string"
                },
                "metric": {
                  "maxLength": 2048,
                  "minLength": 1,
                  "type": "string"
                },
                "population": {
                  "maxLength": 2048,
                  "minLength": 1,
                  "type": "string"
                },
                "statement": {
                  "maxLength": 2048,
                  "minLength": 1,
                  "type": "string"
                }
              },
              "required": [
                "statement",
                "analysis",
                "population",
                "metric",
                "aliases"
              ],
              "type": "object"
            }
          },
          "required": [
            "deliverables"
          ],
          "type": "object"
        }
      },
      "required": [
        "goal"
      ],
      "type": "object"
    },
    "owner": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "plan": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "sealed": {
              "type": "boolean"
            },
            "units": {
              "items": {
                "additionalProperties": false,
                "properties": {
                  "depends_on": {
                    "items": {
                      "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                      "type": "string"
                    },
                    "maxItems": 64,
                    "type": "array",
                    "uniqueItems": true
                  },
                  "execution": {
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
                      "kind": {
                        "type": "string"
                      },
                      "output_storage": {
                        "default": "data/0",
                        "description": "Registered output store from system.status.data_stores. Defaults to data/0; frozen per Job.",
                        "pattern": "^data/[0-9]+$",
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
                      "queue_position": {
                        "default": "tail",
                        "enum": [
                          "tail",
                          "next"
                        ],
                        "type": "string"
                      }
                    },
                    "required": [
                      "kind",
                      "params"
                    ],
                    "type": "object"
                  },
                  "id": {
                    "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                    "type": "string"
                  },
                  "label": {
                    "maxLength": 200,
                    "minLength": 1,
                    "type": "string"
                  },
                  "requires": {
                    "items": {
                      "additionalProperties": false,
                      "properties": {
                        "runtime": {
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
                            },
                            "sha256": {
                              "pattern": "^[a-f0-9]{64}$",
                              "type": "string"
                            }
                          },
                          "required": [
                            "id",
                            "revision"
                          ],
                          "type": "object"
                        },
                        "stage": {
                          "enum": [
                            "prepare",
                            "runtime",
                            "finalize",
                            "on_complete",
                            "on_error",
                            "on_cancel"
                          ]
                        }
                      },
                      "required": [
                        "stage",
                        "runtime"
                      ],
                      "type": "object"
                    },
                    "maxItems": 6,
                    "type": "array",
                    "uniqueItems": true
                  }
                },
                "required": [
                  "id",
                  "label"
                ],
                "type": "object"
              },
              "maxItems": 64,
              "type": "array"
            }
          },
          "required": [
            "sealed",
            "units"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "run": {
              "additionalProperties": false,
              "properties": {
                "execution": {
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
                    "kind": {
                      "type": "string"
                    },
                    "output_storage": {
                      "default": "data/0",
                      "description": "Registered output store from system.status.data_stores. Defaults to data/0; frozen per Job.",
                      "pattern": "^data/[0-9]+$",
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
                    "queue_position": {
                      "default": "tail",
                      "enum": [
                        "tail",
                        "next"
                      ],
                      "type": "string"
                    }
                  },
                  "required": [
                    "kind",
                    "params"
                  ],
                  "type": "object"
                },
                "label": {
                  "maxLength": 200,
                  "minLength": 1,
                  "type": "string"
                },
                "requires": {
                  "items": {
                    "additionalProperties": false,
                    "properties": {
                      "runtime": {
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
                          },
                          "sha256": {
                            "pattern": "^[a-f0-9]{64}$",
                            "type": "string"
                          }
                        },
                        "required": [
                          "id",
                          "revision"
                        ],
                        "type": "object"
                      },
                      "stage": {
                        "enum": [
                          "prepare",
                          "runtime",
                          "finalize",
                          "on_complete",
                          "on_error",
                          "on_cancel"
                        ]
                      }
                    },
                    "required": [
                      "stage",
                      "runtime"
                    ],
                    "type": "object"
                  },
                  "maxItems": 6,
                  "type": "array",
                  "uniqueItems": true
                }
              },
              "required": [
                "label"
              ],
              "type": "object"
            },
            "sealed": {
              "type": "boolean"
            }
          },
          "required": [
            "sealed"
          ],
          "type": "object"
        }
      ]
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
    "owner",
    "notify",
    "outcome",
    "plan"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "key": "dataset-generation",
  "notify": "researcher",
  "outcome": {
    "deliverables": [
      {
        "alias": "cases",
        "entity": "cases",
        "kind": "dataset"
      }
    ],
    "goal": "Publish generated cases"
  },
  "owner": "researcher",
  "plan": {
    "run": {
      "label": "Generate",
      "requires": [
        {
          "runtime": {
            "id": "simulation",
            "revision": 1
          },
          "stage": "runtime"
        }
      ]
    },
    "sealed": false
  },
  "scenario_ref": {
    "id": "<task-id>",
    "revision": 1
  }
}
```



### Result and failures

Reservation receipt · id, revision, sealed, execution_job, receipts and science_digest. Exact historical requests retain their response format.

409 on changed replay, incompatible Profile output or unavailable required Runtime.

## revise_plan

Bind or seal a jobs.create reservation on its existing ID. Replay an uncertain revision using the same key and arguments. Use jobs.revise_research for a typed research Job.

Scope: `submit`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/plans.rs#write_plan](../../rust/crates/core/src/plans.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `expected_revision` | yes | `"integer"` | `—` |
| `id` | yes | `"string"` | `—` |
| `key` | yes | `"string"` | `—` |
| `plan` | yes | `"see schema"` | `—` |
| `reason` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "expected_revision": {
      "maximum": 9223372036854775806,
      "minimum": 1,
      "type": "integer"
    },
    "id": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "key": {
      "maxLength": 128,
      "minLength": 1,
      "type": "string"
    },
    "plan": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "sealed": {
              "type": "boolean"
            },
            "units": {
              "items": {
                "additionalProperties": false,
                "properties": {
                  "depends_on": {
                    "items": {
                      "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                      "type": "string"
                    },
                    "maxItems": 64,
                    "type": "array",
                    "uniqueItems": true
                  },
                  "execution": {
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
                      "kind": {
                        "type": "string"
                      },
                      "output_storage": {
                        "default": "data/0",
                        "description": "Registered output store from system.status.data_stores. Defaults to data/0; frozen per Job.",
                        "pattern": "^data/[0-9]+$",
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
                      "queue_position": {
                        "default": "tail",
                        "enum": [
                          "tail",
                          "next"
                        ],
                        "type": "string"
                      }
                    },
                    "required": [
                      "kind",
                      "params"
                    ],
                    "type": "object"
                  },
                  "id": {
                    "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                    "type": "string"
                  },
                  "label": {
                    "maxLength": 200,
                    "minLength": 1,
                    "type": "string"
                  },
                  "requires": {
                    "items": {
                      "additionalProperties": false,
                      "properties": {
                        "runtime": {
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
                            },
                            "sha256": {
                              "pattern": "^[a-f0-9]{64}$",
                              "type": "string"
                            }
                          },
                          "required": [
                            "id",
                            "revision"
                          ],
                          "type": "object"
                        },
                        "stage": {
                          "enum": [
                            "prepare",
                            "runtime",
                            "finalize",
                            "on_complete",
                            "on_error",
                            "on_cancel"
                          ]
                        }
                      },
                      "required": [
                        "stage",
                        "runtime"
                      ],
                      "type": "object"
                    },
                    "maxItems": 6,
                    "type": "array",
                    "uniqueItems": true
                  }
                },
                "required": [
                  "id",
                  "label"
                ],
                "type": "object"
              },
              "maxItems": 64,
              "type": "array"
            }
          },
          "required": [
            "sealed",
            "units"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "run": {
              "additionalProperties": false,
              "properties": {
                "execution": {
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
                    "kind": {
                      "type": "string"
                    },
                    "output_storage": {
                      "default": "data/0",
                      "description": "Registered output store from system.status.data_stores. Defaults to data/0; frozen per Job.",
                      "pattern": "^data/[0-9]+$",
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
                    "queue_position": {
                      "default": "tail",
                      "enum": [
                        "tail",
                        "next"
                      ],
                      "type": "string"
                    }
                  },
                  "required": [
                    "kind",
                    "params"
                  ],
                  "type": "object"
                },
                "label": {
                  "maxLength": 200,
                  "minLength": 1,
                  "type": "string"
                },
                "requires": {
                  "items": {
                    "additionalProperties": false,
                    "properties": {
                      "runtime": {
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
                          },
                          "sha256": {
                            "pattern": "^[a-f0-9]{64}$",
                            "type": "string"
                          }
                        },
                        "required": [
                          "id",
                          "revision"
                        ],
                        "type": "object"
                      },
                      "stage": {
                        "enum": [
                          "prepare",
                          "runtime",
                          "finalize",
                          "on_complete",
                          "on_error",
                          "on_cancel"
                        ]
                      }
                    },
                    "required": [
                      "stage",
                      "runtime"
                    ],
                    "type": "object"
                  },
                  "maxItems": 6,
                  "type": "array",
                  "uniqueItems": true
                }
              },
              "required": [
                "label"
              ],
              "type": "object"
            },
            "sealed": {
              "type": "boolean"
            }
          },
          "required": [
            "sealed"
          ],
          "type": "object"
        }
      ]
    },
    "reason": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    }
  },
  "required": [
    "id",
    "key",
    "expected_revision",
    "reason",
    "plan"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "expected_revision": 1,
  "id": "<outcome-id>",
  "key": "bind-generator",
  "plan": {
    "run": {
      "execution": {
        "kind": "generation.r1",
        "params": {}
      },
      "label": "Generate",
      "requires": [
        {
          "runtime": {
            "id": "simulation",
            "revision": 1
          },
          "stage": "runtime"
        }
      ]
    },
    "sealed": true
  },
  "reason": "Generator Profile and Runtime validated"
}
```



### Result and failures

Reservation receipt · id, revision, sealed, execution_job, receipts and science_digest. Exact historical requests retain their response format.

403 for another principal. 409 on stale revision, changed replay, terminal outcome or unsatisfied prerequisite.

## submit_research

Admit one Regular output and its Units. scenario_ref pins a registered Task ID and revision. A Policy deliverable can set revises to the exact current Regular Policy to publish the next revision with the same ID, owner and Task. An Experiment deliverable can set supplements to the exact current Regular Experiment ID and revision. Publication checks the accepted inputs, evaluation protocol, reset cohort, selection rule and new checkpoint steps before adding its next revision. Scientific Policy outputs require the exact Dataset and Experiment outputs require the exact evaluation Dataset and Policy under the same pinned Task.

Scope: `submit`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/plans.rs#write_research_plan](../../rust/crates/core/src/plans.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `category` | no | `["sdg", "data_preparation", "fine_tuning", "evaluation", "pipeline", "development", "other"]` | `—` |
| `key` | yes | `"string"` | `—` |
| `notify` | yes | `"string"` | `—` |
| `outcome` | yes | `"object"` | `—` |
| `owner` | yes | `"string"` | `—` |
| `plan` | yes | `"object"` | `—` |
| `presentation` | no | `"object"` | `—` |
| `scenario_ref` | yes | `"object"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "category": {
      "enum": [
        "sdg",
        "data_preparation",
        "fine_tuning",
        "evaluation",
        "pipeline",
        "development",
        "other"
      ]
    },
    "key": {
      "maxLength": 128,
      "minLength": 1,
      "type": "string"
    },
    "notify": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "outcome": {
      "additionalProperties": false,
      "properties": {
        "deliverables": {
          "items": {
            "additionalProperties": false,
            "properties": {
              "alias": {
                "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                "type": "string"
              },
              "from": {
                "additionalProperties": false,
                "properties": {
                  "entity": {
                    "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                    "type": "string"
                  },
                  "unit": {
                    "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                    "type": "string"
                  }
                },
                "required": [
                  "unit",
                  "entity"
                ],
                "type": "object"
              },
              "kind": {
                "enum": [
                  "dataset",
                  "model",
                  "experiment"
                ]
              },
              "revises": {
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
              "supplements": {
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
              "alias",
              "kind",
              "from"
            ],
            "type": "object"
          },
          "maxItems": 1,
          "minItems": 1,
          "type": "array"
        },
        "goal": {
          "maxLength": 200,
          "minLength": 1,
          "type": "string"
        },
        "science": {
          "additionalProperties": false,
          "properties": {
            "deliverables": {
              "additionalProperties": {
                "additionalProperties": false,
                "properties": {
                  "activity": {
                    "maxLength": 2048,
                    "minLength": 1,
                    "type": "string"
                  },
                  "contract_ref": {
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
                  },
                  "inputs": {
                    "additionalProperties": {
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
                    "maxProperties": 64,
                    "type": "object"
                  },
                  "sessions": {
                    "items": {
                      "additionalProperties": false,
                      "properties": {
                        "configuration": {
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
                        },
                        "domain": {
                          "enum": [
                            "simulation",
                            "real"
                          ]
                        },
                        "id": {
                          "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                          "type": "string"
                        }
                      },
                      "required": [
                        "id",
                        "domain",
                        "configuration"
                      ],
                      "type": "object"
                    },
                    "maxItems": 64,
                    "type": "array"
                  },
                  "template_ref": {
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
                  "activity",
                  "template_ref",
                  "contract_ref"
                ],
                "type": "object"
              },
              "maxProperties": 64,
              "minProperties": 1,
              "type": "object"
            },
            "hypothesis": {
              "additionalProperties": false,
              "properties": {
                "aliases": {
                  "items": {
                    "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                    "type": "string"
                  },
                  "maxItems": 64,
                  "minItems": 1,
                  "type": "array",
                  "uniqueItems": true
                },
                "analysis": {
                  "maxLength": 2048,
                  "minLength": 1,
                  "type": "string"
                },
                "metric": {
                  "maxLength": 2048,
                  "minLength": 1,
                  "type": "string"
                },
                "population": {
                  "maxLength": 2048,
                  "minLength": 1,
                  "type": "string"
                },
                "statement": {
                  "maxLength": 2048,
                  "minLength": 1,
                  "type": "string"
                }
              },
              "required": [
                "statement",
                "analysis",
                "population",
                "metric",
                "aliases"
              ],
              "type": "object"
            }
          },
          "required": [
            "deliverables"
          ],
          "type": "object"
        }
      },
      "required": [
        "goal",
        "deliverables"
      ],
      "type": "object"
    },
    "owner": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "plan": {
      "additionalProperties": false,
      "properties": {
        "sealed": {
          "type": "boolean"
        },
        "units": {
          "items": {
            "additionalProperties": false,
            "properties": {
              "depends_on": {
                "items": {
                  "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                  "type": "string"
                },
                "maxItems": 64,
                "type": "array",
                "uniqueItems": true
              },
              "execution": {
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
                  "kind": {
                    "type": "string"
                  },
                  "output_storage": {
                    "default": "data/0",
                    "description": "Registered output store from system.status.data_stores. Defaults to data/0; frozen per Job.",
                    "pattern": "^data/[0-9]+$",
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
                  "queue_position": {
                    "default": "tail",
                    "enum": [
                      "tail",
                      "next"
                    ],
                    "type": "string"
                  }
                },
                "required": [
                  "kind",
                  "params"
                ],
                "type": "object"
              },
              "id": {
                "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                "type": "string"
              },
              "label": {
                "maxLength": 200,
                "minLength": 1,
                "type": "string"
              },
              "phase": {
                "enum": [
                  "prepare",
                  "run",
                  "finalize"
                ]
              },
              "requires": {
                "items": {
                  "additionalProperties": false,
                  "properties": {
                    "runtime": {
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
                        },
                        "sha256": {
                          "pattern": "^[a-f0-9]{64}$",
                          "type": "string"
                        }
                      },
                      "required": [
                        "id",
                        "revision"
                      ],
                      "type": "object"
                    },
                    "stage": {
                      "enum": [
                        "prepare",
                        "runtime",
                        "finalize",
                        "on_complete",
                        "on_error",
                        "on_cancel"
                      ]
                    }
                  },
                  "required": [
                    "stage",
                    "runtime"
                  ],
                  "type": "object"
                },
                "maxItems": 6,
                "type": "array",
                "uniqueItems": true
              }
            },
            "required": [
              "id",
              "label"
            ],
            "type": "object"
          },
          "maxItems": 64,
          "minItems": 1,
          "type": "array"
        }
      },
      "required": [
        "sealed",
        "units"
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
    "owner",
    "notify",
    "outcome",
    "plan",
    "scenario_ref"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "key": "generate-paired",
  "notify": "owner",
  "outcome": {
    "deliverables": [
      {
        "alias": "dataset",
        "from": {
          "entity": "dataset",
          "unit": "generate"
        },
        "kind": "dataset"
      }
    ],
    "goal": "Publish the final Dataset"
  },
  "owner": "owner",
  "plan": {
    "sealed": true,
    "units": [
      {
        "execution": {
          "kind": "owner.collect.r1",
          "params": {}
        },
        "id": "generate",
        "label": "Generate"
      }
    ]
  },
  "scenario_ref": {
    "id": "<task-id>",
    "revision": 1
  }
}
```



### Result and failures

Returns the accepted Job ID, plan revision, sealed flag and map of Unit names to independently readable execution IDs.

Invalid inputs, pinned software, resource request or insufficient scope.

## revise_research

Bind or add Units within the original accepted research outcome.

Scope: `submit`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/plans.rs#write_research_plan](../../rust/crates/core/src/plans.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `expected_revision` | yes | `"integer"` | `—` |
| `id` | yes | `"string"` | `—` |
| `key` | yes | `"string"` | `—` |
| `plan` | yes | `"object"` | `—` |
| `reason` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "expected_revision": {
      "maximum": 9223372036854775806,
      "minimum": 1,
      "type": "integer"
    },
    "id": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "key": {
      "maxLength": 128,
      "minLength": 1,
      "type": "string"
    },
    "plan": {
      "additionalProperties": false,
      "properties": {
        "sealed": {
          "type": "boolean"
        },
        "units": {
          "items": {
            "additionalProperties": false,
            "properties": {
              "depends_on": {
                "items": {
                  "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                  "type": "string"
                },
                "maxItems": 64,
                "type": "array",
                "uniqueItems": true
              },
              "execution": {
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
                  "kind": {
                    "type": "string"
                  },
                  "output_storage": {
                    "default": "data/0",
                    "description": "Registered output store from system.status.data_stores. Defaults to data/0; frozen per Job.",
                    "pattern": "^data/[0-9]+$",
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
                  "queue_position": {
                    "default": "tail",
                    "enum": [
                      "tail",
                      "next"
                    ],
                    "type": "string"
                  }
                },
                "required": [
                  "kind",
                  "params"
                ],
                "type": "object"
              },
              "id": {
                "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                "type": "string"
              },
              "label": {
                "maxLength": 200,
                "minLength": 1,
                "type": "string"
              },
              "phase": {
                "enum": [
                  "prepare",
                  "run",
                  "finalize"
                ]
              },
              "requires": {
                "items": {
                  "additionalProperties": false,
                  "properties": {
                    "runtime": {
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
                        },
                        "sha256": {
                          "pattern": "^[a-f0-9]{64}$",
                          "type": "string"
                        }
                      },
                      "required": [
                        "id",
                        "revision"
                      ],
                      "type": "object"
                    },
                    "stage": {
                      "enum": [
                        "prepare",
                        "runtime",
                        "finalize",
                        "on_complete",
                        "on_error",
                        "on_cancel"
                      ]
                    }
                  },
                  "required": [
                    "stage",
                    "runtime"
                  ],
                  "type": "object"
                },
                "maxItems": 6,
                "type": "array",
                "uniqueItems": true
              }
            },
            "required": [
              "id",
              "label"
            ],
            "type": "object"
          },
          "maxItems": 64,
          "minItems": 1,
          "type": "array"
        }
      },
      "required": [
        "sealed",
        "units"
      ],
      "type": "object"
    },
    "reason": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    }
  },
  "required": [
    "id",
    "key",
    "expected_revision",
    "reason",
    "plan"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "expected_revision": 1,
  "id": "<job-id>",
  "key": "bind-generator",
  "plan": {
    "sealed": true,
    "units": [
      {
        "execution": {
          "kind": "owner.collect.r1",
          "params": {}
        },
        "id": "generate",
        "label": "Generate"
      }
    ]
  },
  "reason": "Verified Runtime"
}
```



### Result and failures

Accepted identity and execution details

Invalid inputs, pinned software, resource request or insufficient scope.

## preview_outcome

Check output kinds, reporting pins, setup and optional hypothesis before submission. Direct Job deliverables identify the output entity.

Scope: `read`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/plans.rs#preview_outcome](../../rust/crates/core/src/plans.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `outcome` | yes | `"object"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "outcome": {
      "additionalProperties": false,
      "properties": {
        "deliverables": {
          "items": {
            "oneOf": [
              {
                "additionalProperties": false,
                "properties": {
                  "alias": {
                    "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                    "type": "string"
                  },
                  "from": {
                    "additionalProperties": false,
                    "properties": {
                      "entity": {
                        "type": "string"
                      },
                      "node": {
                        "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                        "type": "string"
                      }
                    },
                    "required": [
                      "node",
                      "entity"
                    ],
                    "type": "object"
                  },
                  "kind": {
                    "enum": [
                      "artifact",
                      "dataset",
                      "model",
                      "experiment"
                    ]
                  }
                },
                "required": [
                  "alias",
                  "kind",
                  "from"
                ],
                "type": "object"
              },
              {
                "additionalProperties": false,
                "properties": {
                  "alias": {
                    "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                    "type": "string"
                  },
                  "kind": {
                    "const": "runtime_receipt"
                  },
                  "runtime": {
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
                      },
                      "sha256": {
                        "pattern": "^[a-f0-9]{64}$",
                        "type": "string"
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
                  "alias",
                  "kind",
                  "runtime"
                ],
                "type": "object"
              },
              {
                "additionalProperties": false,
                "properties": {
                  "alias": {
                    "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                    "type": "string"
                  },
                  "entity": {
                    "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                    "type": "string"
                  },
                  "kind": {
                    "enum": [
                      "artifact",
                      "dataset",
                      "model",
                      "experiment"
                    ]
                  }
                },
                "required": [
                  "alias",
                  "entity",
                  "kind"
                ],
                "type": "object"
              }
            ]
          },
          "maxItems": 256,
          "type": "array"
        },
        "goal": {
          "maxLength": 200,
          "minLength": 1,
          "type": "string"
        },
        "science": {
          "additionalProperties": false,
          "properties": {
            "deliverables": {
              "additionalProperties": {
                "additionalProperties": false,
                "properties": {
                  "activity": {
                    "maxLength": 2048,
                    "minLength": 1,
                    "type": "string"
                  },
                  "contract_ref": {
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
                  },
                  "inputs": {
                    "additionalProperties": {
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
                    "maxProperties": 64,
                    "type": "object"
                  },
                  "sessions": {
                    "items": {
                      "additionalProperties": false,
                      "properties": {
                        "configuration": {
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
                        },
                        "domain": {
                          "enum": [
                            "simulation",
                            "real"
                          ]
                        },
                        "id": {
                          "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                          "type": "string"
                        }
                      },
                      "required": [
                        "id",
                        "domain",
                        "configuration"
                      ],
                      "type": "object"
                    },
                    "maxItems": 64,
                    "type": "array"
                  },
                  "template_ref": {
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
                  "activity",
                  "template_ref",
                  "contract_ref"
                ],
                "type": "object"
              },
              "maxProperties": 64,
              "minProperties": 1,
              "type": "object"
            },
            "hypothesis": {
              "additionalProperties": false,
              "properties": {
                "aliases": {
                  "items": {
                    "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$",
                    "type": "string"
                  },
                  "maxItems": 64,
                  "minItems": 1,
                  "type": "array",
                  "uniqueItems": true
                },
                "analysis": {
                  "maxLength": 2048,
                  "minLength": 1,
                  "type": "string"
                },
                "metric": {
                  "maxLength": 2048,
                  "minLength": 1,
                  "type": "string"
                },
                "population": {
                  "maxLength": 2048,
                  "minLength": 1,
                  "type": "string"
                },
                "statement": {
                  "maxLength": 2048,
                  "minLength": 1,
                  "type": "string"
                }
              },
              "required": [
                "statement",
                "analysis",
                "population",
                "metric",
                "aliases"
              ],
              "type": "object"
            }
          },
          "required": [
            "deliverables"
          ],
          "type": "object"
        }
      },
      "required": [
        "goal"
      ],
      "type": "object"
    }
  },
  "required": [
    "outcome"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "outcome": {
    "goal": "Verify the requested hardware diagnostic"
  }
}
```



### Result and failures

Preview · mode, per-alias coverage and science_digest

400 for inconsistent deliverable or hypothesis aliases. 409 for mismatched pins, activity, inputs or setup domains.

## preview_history

Read authority for up to 24 exact Job IDs. Inspect workflow membership separately from a presentation parent, a preserved snapshot, and an exact consolidation. This review never infers Steps or writes Job history.

Scope: `read`. Owner: [queue](../features/queue.md). Implementation: [rust/crates/core/src/query.rs#preview_job_history](../../rust/crates/core/src/query.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `ids` | yes | `"array"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "ids": {
      "items": {
        "maxLength": 2048,
        "minLength": 1,
        "type": "string"
      },
      "maxItems": 24,
      "minItems": 1,
      "type": "array",
      "uniqueItems": true
    }
  },
  "required": [
    "ids"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "ids": [
    "<job-id>"
  ]
}
```



### Result and failures

Bounded Job review · records, changes

404 if a selected exact Job is unavailable.

## prune_logs

Expire explicit returned and closed stream generations, preserve gap evidence and remove their searchable/raw captured bytes. Does not remove Jobs or containers.

Scope: `submit`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/maintenance.rs#maintenance](../../rust/crates/core/src/maintenance.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `key` | yes | `"string"` | `—` |
| `reason` | yes | `"string"` | `—` |
| `streams` | yes | `"array"` | `—` |

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
    "reason": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "streams": {
      "items": {
        "additionalProperties": false,
        "properties": {
          "source_generation": {
            "type": "string"
          },
          "stream_id": {
            "type": "string"
          }
        },
        "required": [
          "stream_id",
          "source_generation"
        ],
        "type": "object"
      },
      "maxItems": 200,
      "minItems": 1,
      "type": "array"
    }
  },
  "required": [
    "key",
    "reason",
    "streams"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "key": "retention-1",
  "reason": "Explicit retention policy",
  "streams": [
    {
      "source_generation": "019ac000-0000-7000-8000-000000000000",
      "stream_id": "2156055679491977121222771780406505881"
    }
  ]
}
```



### Result and failures

Retention maintenance task · id, kind, state, progress, receipt, error

Live/unreturned source, unknown generation, unavailable raw bytes or changed request key.

## log_streams

Discover exact Attempt/stage/effect streams for an existing Job without creating work or acknowledging notifications.

Scope: `read`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/log_index.rs#log_streams](../../rust/crates/core/src/log_index.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `after` | no | `"string"` | `—` |
| `attempt` | no | `"string"` | `—` |
| `id` | yes | `"string"` | `—` |
| `limit` | no | `"integer"` | `—` |
| `stage` | no | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "after": {
      "pattern": "^[0-9A-F]{32}$",
      "type": "string"
    },
    "attempt": {
      "type": "string"
    },
    "id": {
      "type": "string"
    },
    "limit": {
      "maximum": 200,
      "minimum": 1,
      "type": "integer"
    },
    "stage": {
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
  "id": "<job-id>"
}
```



### Result and failures

Stream page · items, next_after, partial, warnings

Unknown Job or unavailable exact stream metadata.

## search_logs

Find indexed text within a Job or exact stream. Page the pinned index snapshot and follow raw offset references.

Scope: `read`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/log_index.rs#search_logs](../../rust/crates/core/src/log_index.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `after` | no | `"string"` | `—` |
| `attempt` | no | `"string"` | `—` |
| `cursor` | no | `"string"` | `—` |
| `id` | no | `"string"` | `—` |
| `limit` | no | `"integer"` | `—` |
| `stage` | no | `"string"` | `—` |
| `stream_id` | no | `"string"` | `—` |
| `text` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "after": {
      "pattern": "^[0-9A-F]{32}$",
      "type": "string"
    },
    "attempt": {
      "type": "string"
    },
    "cursor": {
      "maxLength": 4096,
      "type": "string"
    },
    "id": {
      "type": "string"
    },
    "limit": {
      "maximum": 200,
      "minimum": 1,
      "type": "integer"
    },
    "stage": {
      "type": "string"
    },
    "stream_id": {
      "pattern": "^[1-9][0-9]{0,38}$",
      "type": "string"
    },
    "text": {
      "maxLength": 1024,
      "minLength": 3,
      "type": "string"
    }
  },
  "required": [
    "text"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "id": "<job-id>",
  "text": "error"
}
```



### Result and failures

Log search page · items with raw offset references, next_cursor, partial, consistency, warnings

Query too broad, unavailable index/source, expired cursor or retained-byte gap.

## read_log

Read raw byte ranges or bounded tail. Keep source_generation with offsets; report closed, gap and unavailable states.

Scope: `read`. Owner: [catalog](../features/catalog.md). Implementation: [rust/crates/core/src/log_index.rs#read_log](../../rust/crates/core/src/log_index.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `limit` | no | `"integer"` | `—` |
| `offset` | no | `"string"` | `—` |
| `source_generation` | no | `"string"` | `—` |
| `stream_id` | yes | `"string"` | `—` |
| `tail` | no | `"boolean"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "limit": {
      "maximum": 262144,
      "minimum": 1,
      "type": "integer"
    },
    "offset": {
      "pattern": "^[0-9]{1,20}$",
      "type": "string"
    },
    "source_generation": {
      "maxLength": 128,
      "type": "string"
    },
    "stream_id": {
      "pattern": "^[1-9][0-9]{0,38}$",
      "type": "string"
    },
    "tail": {
      "type": "boolean"
    }
  },
  "required": [
    "stream_id"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "limit": 4096,
  "offset": "0",
  "stream_id": "12345678901234567890"
}
```



### Result and failures

Log range · stream_id, source_generation, raw offsets, data_base64, text, eof, closed, gap

Unknown stream, changed source generation, expired retention or unavailable raw bytes.
