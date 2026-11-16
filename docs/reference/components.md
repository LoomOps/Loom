# components

Profiles, Components, Tools and Runtimes

The examples are argument shapes, not live instructions. Replace symbolic identities and hashes with discovered exact records and satisfy the handler preconditions before a mutation. Nested schema constraints are retained below; response notes are descriptive, not a formal response schema.

## Actions

- [list](#list)
- [get](#get)
- [register](#register)
- [write](#write)
- [profiles](#profiles)
- [register_profile](#register_profile)
- [get_profile](#get_profile)
- [list_tools](#list_tools)
- [get_tool](#get_tool)
- [register_tool](#register_tool)
- [runtimes](#runtimes)
- [get_runtime](#get_runtime)
- [dependencies](#dependencies)
- [get_dependency](#get_dependency)
- [assign_owner](#assign_owner)
- [annotate](#annotate)
- [curate](#curate)
- [inspect_profile](#inspect_profile)
- [compare_profiles](#compare_profiles)
- [scaffold](#scaffold)
- [validate_package](#validate_package)

## list

Read the latest revision of all components. Set owner to filter the namespace.

Scope: `read`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/software.rs#software_action](../../rust/crates/core/src/software.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `owner` | no | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "owner": {
      "description": "Stable namespace. Register/write/get default to the authenticated principal; list defaults to all shared namespaces. built-in denotes Loom-supplied software, not shared access. Display responsibility is returned in owner_profile; assign_owner changes attribution without moving this namespace.",
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



### Result and failures

Component[] · owner (namespace), id, revision, owner_profile, owner_name, owner_kind

Shared authentication, schema, scope and handler preconditions still apply.

## get

Read an exact or latest Job lifecycle Component manifest and profiles.

Scope: `read`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/software.rs#software_action](../../rust/crates/core/src/software.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `owner` | no | `"string"` | `—` |
| `revision` | no | `"integer"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "pattern": "[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}",
      "type": "string"
    },
    "owner": {
      "description": "Stable namespace. Register/write/get default to the authenticated principal; list defaults to all shared namespaces. built-in denotes Loom-supplied software, not shared access. Display responsibility is returned in owner_profile; assign_owner changes attribution without moving this namespace.",
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
  "id": "<component-id>"
}
```



### Result and failures

Component · owner namespace, id, revision, manifest, profiles and current attribution

Missing Component revision or read scope.

## register

Register source as an immutable revision. profiles is optional; standalone register_profile can reference the revision. New orchestration defaults to Rust; discover Cargo/SDK templates with components.scaffold. Registration never builds or executes source.

Scope: `submit`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/software.rs#software_action](../../rust/crates/core/src/software.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `files` | no | `"object"` | `—` |
| `id` | yes | `"string"` | `—` |
| `manifest` | yes | `"object"` | `—` |
| `owner` | no | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "files": {
      "additionalProperties": {
        "type": "string"
      },
      "description": "Relative path → UTF-8 source; omit to use saved editable sources.",
      "type": "object"
    },
    "id": {
      "pattern": "[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}",
      "type": "string"
    },
    "manifest": {
      "description": "profiles: name → execution v2 profile.",
      "type": "object"
    },
    "owner": {
      "description": "Stable namespace. Register/write/get default to the authenticated principal; list defaults to all shared namespaces. built-in denotes Loom-supplied software, not shared access. Display responsibility is returned in owner_profile; assign_owner changes attribution without moving this namespace.",
      "type": "string"
    }
  },
  "required": [
    "id",
    "manifest"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "files": {
    "src/lib.rs": "pub fn sample() -> u32 { 1 }"
  },
  "id": "example-rust-package",
  "manifest": {
    "implementation": {
      "language": "rust"
    }
  }
}
```



### Result and failures

Registration · owner (namespace), id, revision, profiles[], owner_profile, owner_name, owner_kind

Missing source, unavailable runtime, missing GPU permission or profile contract mismatch.

## write

Save editable source at a relative path. Active revisions remain unchanged.

Scope: `submit`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/software.rs#software_action](../../rust/crates/core/src/software.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `content` | yes | `"string"` | `—` |
| `id` | yes | `"string"` | `—` |
| `owner` | no | `"string"` | `—` |
| `path` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "content": {
      "type": "string"
    },
    "id": {
      "pattern": "[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}",
      "type": "string"
    },
    "owner": {
      "description": "Stable namespace. Register/write/get default to the authenticated principal; list defaults to all shared namespaces. built-in denotes Loom-supplied software, not shared access. Display responsibility is returned in owner_profile; assign_owner changes attribution without moving this namespace.",
      "type": "string"
    },
    "path": {
      "description": "Relative source path.",
      "type": "string"
    }
  },
  "required": [
    "id",
    "path",
    "content"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "content": "import json, sys\nfrom pathlib import Path\nr = json.loads(Path(sys.argv[-1]).read_text())\no = Path(r[\"output_dir\"])\n(o/\"sample.txt\").write_text(\"hello\")\np = {\"profile\": r[\"profile\"][\"revision\"], \"producer_sha256\": r[\"profile\"][\"sha256\"]}\n(o/\"result.json\").write_text(json.dumps({\"entities\": [{\"id\": \"sample\", \"kind\": \"artifact\", \"path\": \"sample.txt\", \"provenance\": p, \"relations\": []}], \"metrics\": []}))\n",
  "id": "demo",
  "path": "run.py"
}
```



### Result and failures

Editable source receipt · owner, id, path, bytes

Invalid identity, unsafe relative path, missing submit scope or inactive installation.

## profiles

Read exact Profile combinations, stage environments and observed Job usage. Repeated environment dependencies are a bounded preview (dependency_count, dependencies_complete); use components.get_runtime for the complete inventory. Read scope does not grant execution or registration rights.

Scope: `read`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/composition.rs#profile_listing](../../rust/crates/core/src/composition.rs).

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

Profile[] · kind, revision, params_schema, outputs, limits, usage, combination

Shared authentication, schema, scope and handler preconditions still apply.

## register_profile

Register an execution Profile combining optional Component, exact Tools and stage environments. Optional multi-purpose usage is descriptive. Available without redeployment; Jobs pin the revision.

Scope: `submit`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/software.rs#software_action](../../rust/crates/core/src/software.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `component` | no | `"object"` | `—` |
| `id` | yes | `"string"` | `—` |
| `owner` | no | `"string"` | `—` |
| `profile` | yes | `"object"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "component": {
      "additionalProperties": false,
      "properties": {
        "id": {
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
    "id": {
      "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}$",
      "type": "string"
    },
    "owner": {
      "type": "string"
    },
    "profile": {
      "additionalProperties": false,
      "properties": {
        "defaults": {
          "type": "object"
        },
        "finalize": {
          "additionalProperties": false,
          "oneOf": [
            {
              "not": {
                "anyOf": [
                  {
                    "required": [
                      "runtime_id"
                    ]
                  },
                  {
                    "required": [
                      "runtime_ref"
                    ]
                  }
                ]
              },
              "required": [
                "image"
              ]
            },
            {
              "not": {
                "anyOf": [
                  {
                    "required": [
                      "image"
                    ]
                  },
                  {
                    "required": [
                      "runtime_ref"
                    ]
                  }
                ]
              },
              "required": [
                "runtime_id"
              ]
            },
            {
              "not": {
                "anyOf": [
                  {
                    "required": [
                      "image"
                    ]
                  },
                  {
                    "required": [
                      "runtime_id"
                    ]
                  }
                ]
              },
              "required": [
                "runtime_ref"
              ]
            }
          ],
          "properties": {
            "command": {
              "items": {
                "minLength": 1,
                "type": "string"
              },
              "minItems": 1,
              "type": "array"
            },
            "depends_on": {
              "items": {
                "type": "string"
              },
              "type": "array"
            },
            "env": {
              "additionalProperties": {
                "type": "string"
              },
              "type": "object"
            },
            "executor": {
              "const": "container"
            },
            "image": {
              "description": "Cached Docker reference; resolved to image ID when registering.",
              "type": "string"
            },
            "inputs": {
              "items": {
                "type": "string"
              },
              "type": "array"
            },
            "lane": {
              "enum": [
                "cpu",
                "gpu"
              ]
            },
            "resources": {
              "additionalProperties": false,
              "properties": {
                "cpus": {
                  "exclusiveMinimum": 0,
                  "type": "number"
                },
                "memory": {
                  "pattern": "^[1-9][0-9]*[kmg]$",
                  "type": "string"
                },
                "pids": {
                  "minimum": 16,
                  "type": "integer"
                }
              },
              "type": "object"
            },
            "runtime_id": {
              "description": "Optional administrator-installed compatibility runtime.",
              "type": "string"
            },
            "runtime_ref": {
              "additionalProperties": false,
              "properties": {
                "id": {
                  "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}$",
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
            "storage": {
              "description": "Opt-in RW stores from system.status.data_stores; mounted at /loom/data/N. Omit for only the Job workspace.",
              "items": {
                "pattern": "^data/[0-9]+$",
                "type": "string"
              },
              "type": "array",
              "uniqueItems": true
            },
            "timeout": {
              "minimum": 1,
              "type": "integer"
            }
          },
          "required": [
            "executor",
            "command"
          ],
          "type": "object"
        },
        "inputs": {
          "additionalProperties": {
            "type": "string"
          },
          "description": "Input parameter name to allowed data-relative prefix.",
          "type": "object"
        },
        "limits": {
          "additionalProperties": false,
          "properties": {
            "max_attempts": {
              "maximum": 100,
              "minimum": 1,
              "type": "integer"
            },
            "max_output_bytes": {
              "minimum": 1,
              "type": "integer"
            },
            "timeout_seconds": {
              "minimum": 1,
              "type": "integer"
            }
          },
          "required": [
            "timeout_seconds",
            "max_attempts"
          ],
          "type": "object"
        },
        "metadata_schemas": {
          "additionalProperties": {
            "type": [
              "object",
              "boolean"
            ]
          },
          "description": "Output ID or * to local JSON Schema for entity.metadata.",
          "type": "object"
        },
        "on_cancel": {
          "additionalProperties": false,
          "oneOf": [
            {
              "not": {
                "anyOf": [
                  {
                    "required": [
                      "runtime_id"
                    ]
                  },
                  {
                    "required": [
                      "runtime_ref"
                    ]
                  }
                ]
              },
              "required": [
                "image"
              ]
            },
            {
              "not": {
                "anyOf": [
                  {
                    "required": [
                      "image"
                    ]
                  },
                  {
                    "required": [
                      "runtime_ref"
                    ]
                  }
                ]
              },
              "required": [
                "runtime_id"
              ]
            },
            {
              "not": {
                "anyOf": [
                  {
                    "required": [
                      "image"
                    ]
                  },
                  {
                    "required": [
                      "runtime_id"
                    ]
                  }
                ]
              },
              "required": [
                "runtime_ref"
              ]
            }
          ],
          "properties": {
            "command": {
              "items": {
                "minLength": 1,
                "type": "string"
              },
              "minItems": 1,
              "type": "array"
            },
            "depends_on": {
              "items": {
                "type": "string"
              },
              "type": "array"
            },
            "env": {
              "additionalProperties": {
                "type": "string"
              },
              "type": "object"
            },
            "executor": {
              "const": "container"
            },
            "image": {
              "description": "Cached Docker reference; resolved to image ID when registering.",
              "type": "string"
            },
            "inputs": {
              "items": {
                "type": "string"
              },
              "type": "array"
            },
            "lane": {
              "enum": [
                "cpu",
                "gpu"
              ]
            },
            "resources": {
              "additionalProperties": false,
              "properties": {
                "cpus": {
                  "exclusiveMinimum": 0,
                  "type": "number"
                },
                "memory": {
                  "pattern": "^[1-9][0-9]*[kmg]$",
                  "type": "string"
                },
                "pids": {
                  "minimum": 16,
                  "type": "integer"
                }
              },
              "type": "object"
            },
            "runtime_id": {
              "description": "Optional administrator-installed compatibility runtime.",
              "type": "string"
            },
            "runtime_ref": {
              "additionalProperties": false,
              "properties": {
                "id": {
                  "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}$",
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
            "storage": {
              "description": "Opt-in RW stores from system.status.data_stores; mounted at /loom/data/N. Omit for only the Job workspace.",
              "items": {
                "pattern": "^data/[0-9]+$",
                "type": "string"
              },
              "type": "array",
              "uniqueItems": true
            },
            "timeout": {
              "minimum": 1,
              "type": "integer"
            }
          },
          "required": [
            "executor",
            "command"
          ],
          "type": "object"
        },
        "on_complete": {
          "additionalProperties": false,
          "oneOf": [
            {
              "not": {
                "anyOf": [
                  {
                    "required": [
                      "runtime_id"
                    ]
                  },
                  {
                    "required": [
                      "runtime_ref"
                    ]
                  }
                ]
              },
              "required": [
                "image"
              ]
            },
            {
              "not": {
                "anyOf": [
                  {
                    "required": [
                      "image"
                    ]
                  },
                  {
                    "required": [
                      "runtime_ref"
                    ]
                  }
                ]
              },
              "required": [
                "runtime_id"
              ]
            },
            {
              "not": {
                "anyOf": [
                  {
                    "required": [
                      "image"
                    ]
                  },
                  {
                    "required": [
                      "runtime_id"
                    ]
                  }
                ]
              },
              "required": [
                "runtime_ref"
              ]
            }
          ],
          "properties": {
            "command": {
              "items": {
                "minLength": 1,
                "type": "string"
              },
              "minItems": 1,
              "type": "array"
            },
            "depends_on": {
              "items": {
                "type": "string"
              },
              "type": "array"
            },
            "env": {
              "additionalProperties": {
                "type": "string"
              },
              "type": "object"
            },
            "executor": {
              "const": "container"
            },
            "image": {
              "description": "Cached Docker reference; resolved to image ID when registering.",
              "type": "string"
            },
            "inputs": {
              "items": {
                "type": "string"
              },
              "type": "array"
            },
            "lane": {
              "enum": [
                "cpu",
                "gpu"
              ]
            },
            "resources": {
              "additionalProperties": false,
              "properties": {
                "cpus": {
                  "exclusiveMinimum": 0,
                  "type": "number"
                },
                "memory": {
                  "pattern": "^[1-9][0-9]*[kmg]$",
                  "type": "string"
                },
                "pids": {
                  "minimum": 16,
                  "type": "integer"
                }
              },
              "type": "object"
            },
            "runtime_id": {
              "description": "Optional administrator-installed compatibility runtime.",
              "type": "string"
            },
            "runtime_ref": {
              "additionalProperties": false,
              "properties": {
                "id": {
                  "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}$",
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
            "storage": {
              "description": "Opt-in RW stores from system.status.data_stores; mounted at /loom/data/N. Omit for only the Job workspace.",
              "items": {
                "pattern": "^data/[0-9]+$",
                "type": "string"
              },
              "type": "array",
              "uniqueItems": true
            },
            "timeout": {
              "minimum": 1,
              "type": "integer"
            }
          },
          "required": [
            "executor",
            "command"
          ],
          "type": "object"
        },
        "on_error": {
          "additionalProperties": false,
          "oneOf": [
            {
              "not": {
                "anyOf": [
                  {
                    "required": [
                      "runtime_id"
                    ]
                  },
                  {
                    "required": [
                      "runtime_ref"
                    ]
                  }
                ]
              },
              "required": [
                "image"
              ]
            },
            {
              "not": {
                "anyOf": [
                  {
                    "required": [
                      "image"
                    ]
                  },
                  {
                    "required": [
                      "runtime_ref"
                    ]
                  }
                ]
              },
              "required": [
                "runtime_id"
              ]
            },
            {
              "not": {
                "anyOf": [
                  {
                    "required": [
                      "image"
                    ]
                  },
                  {
                    "required": [
                      "runtime_id"
                    ]
                  }
                ]
              },
              "required": [
                "runtime_ref"
              ]
            }
          ],
          "properties": {
            "command": {
              "items": {
                "minLength": 1,
                "type": "string"
              },
              "minItems": 1,
              "type": "array"
            },
            "depends_on": {
              "items": {
                "type": "string"
              },
              "type": "array"
            },
            "env": {
              "additionalProperties": {
                "type": "string"
              },
              "type": "object"
            },
            "executor": {
              "const": "container"
            },
            "image": {
              "description": "Cached Docker reference; resolved to image ID when registering.",
              "type": "string"
            },
            "inputs": {
              "items": {
                "type": "string"
              },
              "type": "array"
            },
            "lane": {
              "enum": [
                "cpu",
                "gpu"
              ]
            },
            "resources": {
              "additionalProperties": false,
              "properties": {
                "cpus": {
                  "exclusiveMinimum": 0,
                  "type": "number"
                },
                "memory": {
                  "pattern": "^[1-9][0-9]*[kmg]$",
                  "type": "string"
                },
                "pids": {
                  "minimum": 16,
                  "type": "integer"
                }
              },
              "type": "object"
            },
            "runtime_id": {
              "description": "Optional administrator-installed compatibility runtime.",
              "type": "string"
            },
            "runtime_ref": {
              "additionalProperties": false,
              "properties": {
                "id": {
                  "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}$",
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
            "storage": {
              "description": "Opt-in RW stores from system.status.data_stores; mounted at /loom/data/N. Omit for only the Job workspace.",
              "items": {
                "pattern": "^data/[0-9]+$",
                "type": "string"
              },
              "type": "array",
              "uniqueItems": true
            },
            "timeout": {
              "minimum": 1,
              "type": "integer"
            }
          },
          "required": [
            "executor",
            "command"
          ],
          "type": "object"
        },
        "outputs": {
          "additionalProperties": {
            "enum": [
              "artifact",
              "dataset",
              "model",
              "experiment"
            ]
          },
          "type": [
            "object",
            "null"
          ]
        },
        "params_schema": {
          "description": "Local JSON Schema for resolved Job params.",
          "type": "object"
        },
        "prepare": {
          "additionalProperties": false,
          "oneOf": [
            {
              "not": {
                "anyOf": [
                  {
                    "required": [
                      "runtime_id"
                    ]
                  },
                  {
                    "required": [
                      "runtime_ref"
                    ]
                  }
                ]
              },
              "required": [
                "image"
              ]
            },
            {
              "not": {
                "anyOf": [
                  {
                    "required": [
                      "image"
                    ]
                  },
                  {
                    "required": [
                      "runtime_ref"
                    ]
                  }
                ]
              },
              "required": [
                "runtime_id"
              ]
            },
            {
              "not": {
                "anyOf": [
                  {
                    "required": [
                      "image"
                    ]
                  },
                  {
                    "required": [
                      "runtime_id"
                    ]
                  }
                ]
              },
              "required": [
                "runtime_ref"
              ]
            }
          ],
          "properties": {
            "command": {
              "items": {
                "minLength": 1,
                "type": "string"
              },
              "minItems": 1,
              "type": "array"
            },
            "depends_on": {
              "items": {
                "type": "string"
              },
              "type": "array"
            },
            "env": {
              "additionalProperties": {
                "type": "string"
              },
              "type": "object"
            },
            "executor": {
              "const": "container"
            },
            "image": {
              "description": "Cached Docker reference; resolved to image ID when registering.",
              "type": "string"
            },
            "inputs": {
              "items": {
                "type": "string"
              },
              "type": "array"
            },
            "lane": {
              "enum": [
                "cpu",
                "gpu"
              ]
            },
            "resources": {
              "additionalProperties": false,
              "properties": {
                "cpus": {
                  "exclusiveMinimum": 0,
                  "type": "number"
                },
                "memory": {
                  "pattern": "^[1-9][0-9]*[kmg]$",
                  "type": "string"
                },
                "pids": {
                  "minimum": 16,
                  "type": "integer"
                }
              },
              "type": "object"
            },
            "runtime_id": {
              "description": "Optional administrator-installed compatibility runtime.",
              "type": "string"
            },
            "runtime_ref": {
              "additionalProperties": false,
              "properties": {
                "id": {
                  "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}$",
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
            "storage": {
              "description": "Opt-in RW stores from system.status.data_stores; mounted at /loom/data/N. Omit for only the Job workspace.",
              "items": {
                "pattern": "^data/[0-9]+$",
                "type": "string"
              },
              "type": "array",
              "uniqueItems": true
            },
            "timeout": {
              "minimum": 1,
              "type": "integer"
            }
          },
          "required": [
            "executor",
            "command"
          ],
          "type": "object"
        },
        "presentation": {
          "additionalProperties": false,
          "properties": {
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
        "runtime": {
          "additionalProperties": false,
          "oneOf": [
            {
              "not": {
                "anyOf": [
                  {
                    "required": [
                      "runtime_id"
                    ]
                  },
                  {
                    "required": [
                      "runtime_ref"
                    ]
                  }
                ]
              },
              "required": [
                "image"
              ]
            },
            {
              "not": {
                "anyOf": [
                  {
                    "required": [
                      "image"
                    ]
                  },
                  {
                    "required": [
                      "runtime_ref"
                    ]
                  }
                ]
              },
              "required": [
                "runtime_id"
              ]
            },
            {
              "not": {
                "anyOf": [
                  {
                    "required": [
                      "image"
                    ]
                  },
                  {
                    "required": [
                      "runtime_id"
                    ]
                  }
                ]
              },
              "required": [
                "runtime_ref"
              ]
            }
          ],
          "properties": {
            "command": {
              "items": {
                "minLength": 1,
                "type": "string"
              },
              "minItems": 1,
              "type": "array"
            },
            "depends_on": {
              "items": {
                "type": "string"
              },
              "type": "array"
            },
            "env": {
              "additionalProperties": {
                "type": "string"
              },
              "type": "object"
            },
            "executor": {
              "const": "container"
            },
            "image": {
              "description": "Cached Docker reference; resolved to image ID when registering.",
              "type": "string"
            },
            "inputs": {
              "items": {
                "type": "string"
              },
              "type": "array"
            },
            "lane": {
              "enum": [
                "cpu",
                "gpu"
              ]
            },
            "resources": {
              "additionalProperties": false,
              "properties": {
                "cpus": {
                  "exclusiveMinimum": 0,
                  "type": "number"
                },
                "memory": {
                  "pattern": "^[1-9][0-9]*[kmg]$",
                  "type": "string"
                },
                "pids": {
                  "minimum": 16,
                  "type": "integer"
                }
              },
              "type": "object"
            },
            "runtime_id": {
              "description": "Optional administrator-installed compatibility runtime.",
              "type": "string"
            },
            "runtime_ref": {
              "additionalProperties": false,
              "properties": {
                "id": {
                  "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}$",
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
            "storage": {
              "description": "Opt-in RW stores from system.status.data_stores; mounted at /loom/data/N. Omit for only the Job workspace.",
              "items": {
                "pattern": "^data/[0-9]+$",
                "type": "string"
              },
              "type": "array",
              "uniqueItems": true
            },
            "timeout": {
              "minimum": 1,
              "type": "integer"
            }
          },
          "required": [
            "executor",
            "command"
          ],
          "type": "object"
        },
        "steps": {
          "items": {
            "additionalProperties": false,
            "properties": {
              "id": {
                "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}$",
                "type": "string"
              },
              "label": {
                "maxLength": 200,
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
          "maxItems": 256,
          "type": "array"
        },
        "tools": {
          "additionalProperties": {
            "additionalProperties": false,
            "properties": {
              "id": {
                "type": "string"
              },
              "owner": {
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
          "description": "Alias to an exact registered Tool owner/id/revision.",
          "type": "object"
        },
        "usage": {
          "additionalProperties": false,
          "properties": {
            "categories": {
              "items": {
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
              "minItems": 1,
              "type": "array",
              "uniqueItems": true
            },
            "description": {
              "maxLength": 2000,
              "type": "string"
            },
            "execution_modes": {
              "items": {
                "enum": [
                  "full",
                  "pilot",
                  "smoke"
                ]
              },
              "minItems": 1,
              "type": "array",
              "uniqueItems": true
            },
            "label": {
              "maxLength": 160,
              "minLength": 1,
              "type": "string"
            }
          },
          "type": "object"
        }
      },
      "required": [
        "params_schema",
        "inputs",
        "outputs",
        "runtime",
        "limits"
      ],
      "type": "object"
    }
  },
  "required": [
    "id",
    "profile"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "component": {
    "id": "demo",
    "revision": 1
  },
  "id": "demo-run",
  "profile": {
    "defaults": {},
    "inputs": {},
    "limits": {
      "max_attempts": 1,
      "timeout_seconds": 60
    },
    "metadata_schemas": {},
    "outputs": null,
    "params_schema": {
      "type": "object"
    },
    "runtime": {
      "command": [
        "/opt/example/bin/producer"
      ],
      "executor": "container",
      "lane": "cpu",
      "runtime_ref": {
        "id": "example-producer-runtime",
        "revision": 1
      }
    },
    "usage": {
      "categories": [
        "pipeline",
        "evaluation"
      ],
      "execution_modes": [
        "pilot",
        "full"
      ]
    }
  }
}
```

The exact Runtime must contain the shown prebuilt Rust executable, and the referenced Component must already exist. Replace these placeholders; for @producer builds follow the authoring guide and verified receipt workflow.

### Result and failures

Profile receipt · owner, id, revision, kind, sha256

Uncached image, resource limit exceeded, unregistered component or invalid schema.

## get_profile

Read a specific or latest revision of an independently registered profile.

Scope: `read`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/software.rs#software_action](../../rust/crates/core/src/software.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `owner` | no | `"string"` | `—` |
| `revision` | no | `"integer"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}$",
      "type": "string"
    },
    "owner": {
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
  "id": "demo-run"
}
```



### Result and failures

Profile · owner namespace, id, revision, profile

Missing Profile revision or read scope.

## list_tools

List shared reusable software independently of Job Components.

Scope: `read`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/software.rs#software_action](../../rust/crates/core/src/software.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `owner` | no | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "owner": {
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



### Result and failures

Tool[]

Shared authentication, schema, scope and handler preconditions still apply.

## get_tool

Read an exact or latest immutable software revision.

Scope: `read`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/software.rs#software_action](../../rust/crates/core/src/software.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `owner` | no | `"string"` | `—` |
| `revision` | no | `"integer"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "pattern": "[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}",
      "type": "string"
    },
    "owner": {
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
  "id": "planner",
  "revision": 1
}
```



### Result and failures

Tool

Missing Tool revision.

## register_tool

Register source or explicitly copy an exact legacy Component revision; existing Jobs and revisions remain unchanged. New orchestration defaults to Rust; discover Cargo/SDK templates with components.scaffold. Registration never builds or executes source.

Scope: `submit`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/software.rs#software_action](../../rust/crates/core/src/software.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `component` | no | `"object"` | `—` |
| `files` | no | `"object"` | `—` |
| `id` | yes | `"string"` | `—` |
| `manifest` | yes | `"object"` | `—` |
| `owner` | no | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "component": {
      "additionalProperties": false,
      "properties": {
        "id": {
          "type": "string"
        },
        "owner": {
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
    "files": {
      "additionalProperties": {
        "type": "string"
      },
      "type": "object"
    },
    "id": {
      "pattern": "[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}",
      "type": "string"
    },
    "manifest": {
      "type": "object"
    },
    "owner": {
      "type": "string"
    }
  },
  "required": [
    "id",
    "manifest"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "files": {
    "src/lib.rs": "pub fn sample() -> u32 { 1 }"
  },
  "id": "example-rust-package",
  "manifest": {
    "implementation": {
      "language": "rust"
    }
  }
}
```



### Result and failures

Tool

Invalid source, missing legacy revision or denied submit scope.

## runtimes

Read administrator-granted execution environments, exact dependencies, validation evidence and Profile/Job usage. Legacy entries are not verification evidence.

Scope: `read`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/software.rs#software_action](../../rust/crates/core/src/software.rs).

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

Runtime catalog

Shared authentication, schema, scope and handler preconditions still apply.

## get_runtime

Read one exact immutable Runtime revision and its consumers. Installation/build is an explicit host-administrator workflow.

Scope: `read`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/software.rs#software_action](../../rust/crates/core/src/software.rs).

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
  "id": "simulation",
  "revision": 1
}
```



### Result and failures

Runtime

Missing revision or instance grant.

## dependencies

Read major software units grouped across current granted Runtimes, with exact versions, distinct installation origins and Runtime links. Full transitive package inventories remain on each Runtime. This is recorded metadata, not filesystem discovery or proof of scientific use.

Scope: `read`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/software.rs#software_action](../../rust/crates/core/src/software.rs).

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

Dependency catalog

Shared authentication, schema, scope and handler preconditions still apply.

## get_dependency

Read all recorded versions and origins of one dependency across granted Runtime revisions, including history.

Scope: `read`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/software.rs#software_action](../../rust/crates/core/src/software.rs).

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
  "id": "dependency-<id>"
}
```



### Result and failures

Dependency

Dependency unavailable or outside instance grant.

## assign_owner

Set the responsible user or built-in attribution for Loom-supplied software. Sharing, namespace, source revisions and Job pins remain unchanged.

Scope: `submit`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/composition.rs#curate_software](../../rust/crates/core/src/composition.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `kind` | yes | `["component", "tool", "profile"]` | `—` |
| `namespace` | yes | `"string"` | `—` |
| `owner` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}$",
      "type": "string"
    },
    "kind": {
      "enum": [
        "component",
        "tool",
        "profile"
      ]
    },
    "namespace": {
      "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}$",
      "type": "string"
    },
    "owner": {
      "description": "Registered responsible user ID or built-in for Loom-supplied software. Does not restrict sharing.",
      "type": "string"
    }
  },
  "required": [
    "kind",
    "namespace",
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
  "id": "planner",
  "kind": "tool",
  "namespace": "research",
  "owner": "<user-profile-id>"
}
```



### Result and failures

Attribution · kind, id, namespace, owner_profile, owner_kind, changed

Unregistered user, missing extension, read-only scope or maintenance fence.

## annotate

Attach evidence-backed revision tags and a concise label without changing source, digests, Profiles or Jobs.

Scope: `submit`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/composition.rs#curate_software](../../rust/crates/core/src/composition.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `evidence` | yes | `"string"` | `—` |
| `id` | yes | `"string"` | `—` |
| `kind` | yes | `["component", "tool"]` | `—` |
| `label` | no | `"string"` | `—` |
| `owner` | yes | `"string"` | `—` |
| `revision` | yes | `"integer"` | `—` |
| `tags` | yes | `"array"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "evidence": {
      "maxLength": 2000,
      "minLength": 1,
      "type": "string"
    },
    "id": {
      "pattern": "[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}",
      "type": "string"
    },
    "kind": {
      "enum": [
        "component",
        "tool"
      ]
    },
    "label": {
      "maxLength": 160,
      "minLength": 1,
      "type": "string"
    },
    "owner": {
      "type": "string"
    },
    "revision": {
      "minimum": 1,
      "type": "integer"
    },
    "tags": {
      "items": {
        "enum": [
          "sdg",
          "training",
          "evaluation",
          "validation",
          "data-processing",
          "motion-planning",
          "policy",
          "control",
          "simulation",
          "rendering",
          "sensing",
          "perception",
          "metrics",
          "packaging",
          "platform-check",
          "utility"
        ]
      },
      "maxItems": 16,
      "type": "array",
      "uniqueItems": true
    }
  },
  "required": [
    "kind",
    "owner",
    "id",
    "revision",
    "tags",
    "evidence"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "evidence": "Verified planner interface in this revision.",
  "id": "planner",
  "kind": "tool",
  "owner": "<namespace>",
  "revision": 1,
  "tags": [
    "motion-planning"
  ]
}
```



### Result and failures

Display metadata

Missing revision, invalid tag, submit scope denied or maintenance fence.

## curate

Set a Profile display name, purpose, preferred/archive state or declared Tool stage roles; set representative dependencies on an exact Runtime. Does not rewrite execution definitions or select a Runtime.

Scope: `submit`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/composition.rs#curate_software](../../rust/crates/core/src/composition.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `categories` | no | `"array"` | `—` |
| `evidence` | yes | `"string"` | `—` |
| `execution_modes` | no | `"array"` | `—` |
| `group` | no | `["research", "platform-check", "other"]` | `—` |
| `id` | yes | `"string"` | `—` |
| `label` | no | `"string"` | `—` |
| `preferred` | no | `"boolean"` | `—` |
| `primary_dependencies` | no | `"array"` | `—` |
| `revision` | no | `"integer"` | `—` |
| `stage_dependencies` | no | `"object"` | `—` |
| `state` | no | `["active", "archived"]` | `—` |
| `tool_roles` | no | `"object"` | `—` |
| `type` | yes | `["profile", "runtime"]` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "categories": {
      "items": {
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
      "type": "array",
      "uniqueItems": true
    },
    "evidence": {
      "maxLength": 2000,
      "minLength": 1,
      "type": "string"
    },
    "execution_modes": {
      "items": {
        "enum": [
          "full",
          "pilot",
          "smoke"
        ]
      },
      "type": "array",
      "uniqueItems": true
    },
    "group": {
      "enum": [
        "research",
        "platform-check",
        "other"
      ]
    },
    "id": {
      "maxLength": 256,
      "minLength": 1,
      "type": "string"
    },
    "label": {
      "maxLength": 100,
      "minLength": 1,
      "type": "string"
    },
    "preferred": {
      "type": "boolean"
    },
    "primary_dependencies": {
      "items": {
        "additionalProperties": false,
        "properties": {
          "name": {
            "maxLength": 160,
            "minLength": 1,
            "type": "string"
          },
          "scope": {
            "maxLength": 1024,
            "type": "string"
          }
        },
        "required": [
          "name"
        ],
        "type": "object"
      },
      "maxItems": 8,
      "type": "array",
      "uniqueItems": true
    },
    "revision": {
      "minimum": 1,
      "type": "integer"
    },
    "stage_dependencies": {
      "additionalProperties": {
        "items": {
          "additionalProperties": false,
          "properties": {
            "name": {
              "maxLength": 160,
              "minLength": 1,
              "type": "string"
            },
            "scope": {
              "maxLength": 1024,
              "type": "string"
            }
          },
          "required": [
            "name"
          ],
          "type": "object"
        },
        "maxItems": 8,
        "type": "array",
        "uniqueItems": true
      },
      "propertyNames": {
        "enum": [
          "prepare",
          "runtime",
          "finalize",
          "on_complete",
          "on_error",
          "on_cancel"
        ]
      },
      "type": "object"
    },
    "state": {
      "enum": [
        "active",
        "archived"
      ]
    },
    "tool_roles": {
      "additionalProperties": {
        "additionalProperties": false,
        "properties": {
          "role": {
            "maxLength": 80,
            "minLength": 1,
            "type": "string"
          },
          "stages": {
            "items": {
              "enum": [
                "prepare",
                "runtime",
                "finalize",
                "on_complete",
                "on_error",
                "on_cancel"
              ]
            },
            "type": "array",
            "uniqueItems": true
          }
        },
        "required": [
          "role",
          "stages"
        ],
        "type": "object"
      },
      "maxProperties": 64,
      "type": "object"
    },
    "type": {
      "enum": [
        "profile",
        "runtime"
      ]
    }
  },
  "required": [
    "type",
    "id",
    "evidence"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "evidence": "Reviewed entrypoint and outputs.",
  "id": "owner.example.run.r1",
  "label": "Evaluation",
  "type": "profile"
}
```



### Result and failures

Curation receipt

Submit scope, maintenance fence, exact identity, dependency scope or preference conflict.

## inspect_profile

Read stage Runtime evidence and scoped dependency requirements; optionally validate parameters. Does not reserve resources, install, import code or submit a Job.

Scope: `read`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/composition.rs#inspect_profiles](../../rust/crates/core/src/composition.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `kind` | yes | `"string"` | `—` |
| `params` | no | `"object"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "kind": {
      "type": "string"
    },
    "params": {
      "type": "object"
    }
  },
  "required": [
    "kind"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "kind": "owner.example.run.r1",
  "params": {}
}
```



### Result and failures

Inspection, parameter validation and partial submission draft

Unknown Profile or invalid inspection arguments.

## compare_profiles

Compare source, stage, Tool/Runtime, parameter and output contracts. Returns changed paths without environment variable values. Display-only changes do not count as execution changes.

Scope: `read`. Owner: [extensions](../features/extensions.md). Implementation: [rust/crates/core/src/composition.rs#inspect_profiles](../../rust/crates/core/src/composition.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `left` | yes | `"string"` | `—` |
| `right` | yes | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "left": {
      "type": "string"
    },
    "right": {
      "type": "string"
    }
  },
  "required": [
    "left",
    "right"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "left": "owner.example.run.r1",
  "right": "owner.example.run.r2"
}
```



### Result and failures

Changed paths and execution_equal

Unknown Profile.

## scaffold

Return the selected Rust Tool/Component/hybrid source draft and vendored SDK. Runtime and external SDK placeholders require selection; this neither builds nor registers software.

Scope: `read`. Owner: [authoring](../features/authoring.md). Implementation: [rust/crates/core/src/software.rs#scaffold](../../rust/crates/core/src/software.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `name` | yes | `"string"` | `—` |
| `template` | yes | `["tool", "component", "hybrid"]` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "name": {
      "maxLength": 2048,
      "minLength": 1,
      "type": "string"
    },
    "template": {
      "enum": [
        "tool",
        "component",
        "hybrid"
      ]
    }
  },
  "required": [
    "template",
    "name"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "name": "example-producer",
  "template": "component"
}
```



### Result and failures

files, manifest, draft_profile, producer_protocol and build_executed=false

Invalid package name.

## validate_package

Check source paths, bounds and declared Python integration metadata. This performs no compilation, import or installation.

Scope: `read`. Owner: [authoring](../features/authoring.md). Implementation: [rust/crates/core/src/software.rs#validate_package](../../rust/crates/core/src/software.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `files` | yes | `"object"` | `—` |
| `manifest` | yes | `"object"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "files": {
      "additionalProperties": {
        "type": "string"
      },
      "type": "object"
    },
    "manifest": {
      "type": "object"
    }
  },
  "required": [
    "manifest",
    "files"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "files": {
    "src/main.rs": "fn main() {}\n"
  },
  "manifest": {
    "implementation": {
      "language": "rust"
    }
  }
}
```



### Result and failures

valid, languages, source_sha256 and build_executed=false

Escaping/conflicting paths, empty source, oversized package or undeclared Python integration.
