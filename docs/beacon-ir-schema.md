# FresnelFir IR Schema Reference

This document defines the complete structure of a FresnelFir Declarative IR specification. AI agents use this schema to construct valid IR JSON for the FresnelFir verification harness.

## Top-Level Structure

A FresnelFir IR document is a JSON object with exactly 9 required top-level sections:

```json
{
  "entities": { ... },
  "refinements": { ... },
  "functions": { ... },
  "protocols": { ... },
  "effects": { ... },
  "properties": { ... },
  "generators": { ... },
  "exploration": { ... },
  "inputs": { ... },
  "bindings": { ... }
}
```

All sections are required. Use empty objects/arrays for unused sections.

---

## Section 1: Entities

Defines the domain model — named entity types with typed fields.

```json
"entities": {
  "<EntityName>": {
    "fields": {
      "<field_name>": <FieldDef>
    }
  }
}
```

### FieldDef Types

| Type | Schema | Notes |
|------|--------|-------|
| `string` | `{ "type": "string", "format": "<optional>" }` | `format` is optional metadata (e.g., `"uuid"`) |
| `bool` | `{ "type": "bool", "default": <optional bool> }` | `default` is optional |
| `int` | `{ "type": "int", "min": <optional i64>, "max": <optional i64> }` | Bounds are optional |
| `enum` | `{ "type": "enum", "values": ["val1", "val2", ...] }` | At least 1 value required |
| `ref` | `{ "type": "ref", "entity": "<EntityName>" }` | Foreign key reference |

**Example:**
```json
"User": {
  "fields": {
    "id": { "type": "string", "format": "uuid" },
    "role": { "type": "enum", "values": ["admin", "member", "guest"] },
    "authenticated": { "type": "bool" }
  }
}
```

---

## Section 2: Refinements

Named subtypes of entities with predicate constraints.

```json
"refinements": {
  "<RefinementName>": {
    "base": "<EntityName>",
    "params": [{ "name": "<param>", "type": "<EntityName>" }],
    "predicate": <Expr>
  }
}
```

- `base` (required): Entity type this refines. Must exist in `entities`.
- `params` (optional, default `[]`): Additional parameters for parameterized refinements.
- `predicate` (required): Expression that must evaluate to `true` for the refinement to hold.

**Example:**
```json
"AuthenticatedUser": {
  "base": "User",
  "predicate": ["eq", ["field", "self", "authenticated"], true]
}
```

---

## Section 3: Functions

Named functions — either `derived` (computed from model state) or `observer` (calls into DUT).

```json
"functions": {
  "<function_name>": {
    "classification": "derived" | "observer",
    "params": [{ "name": "<param>", "type": "<EntityName>" }],
    "body": <Expr>,
    "binding": "<dut_function_name>",
    "returns": "<type_name>"
  }
}
```

- `classification` (required): `"derived"` for model-computed, `"observer"` for DUT-queried.
- `params` (required): Parameter list.
- `body` (optional): Expression body. Required for `derived`, absent for `observer`.
- `binding` (optional): DUT function name. Required for `observer`, absent for `derived`.
- `returns` (required): Return type name.

---

## Section 4: Protocols

Named protocol grammars defining valid operation sequences.

```json
"protocols": {
  "<protocol_name>": {
    "root": <ProtocolNode>
  }
}
```

### ProtocolNode Types

**Seq** — Execute children in order:
```json
{ "type": "seq", "children": [<ProtocolNode>, ...] }
```

**Alt** — Choose one branch (weighted, optionally guarded):
```json
{
  "type": "alt",
  "branches": [
    {
      "id": "<unique_branch_id>",
      "weight": <u32>,
      "guard": <Expr>,
      "body": <ProtocolNode>
    }
  ]
}
```
- `id` (required): Unique identifier for the branch. Used in adaptation.
- `weight` (required): Relative selection probability. Not all weights may be 0.
- `guard` (optional): Expression that must be true for this branch to be eligible.

**Repeat** — Execute body between `min` and `max` times:
```json
{ "type": "repeat", "min": <u32>, "max": <u32>, "body": <ProtocolNode> }
```
- `min` must be <= `max`.

**Call** — Execute a named action:
```json
{ "type": "call", "action": "<action_name>" }
```
- `action` must have a corresponding entry in `effects` and `bindings.actions`.

**Ref** — Reference another protocol:
```json
{ "type": "ref", "protocol": "<protocol_name>" }
```
- Referenced protocol must exist in `protocols`.

---

## Section 5: Effects

Model-side state mutations for each action.

```json
"effects": {
  "<action_name>": {
    "creates": { "entity": "<EntityName>", "assign": "<var_name>" },
    "sets": [
      { "target": ["<var>", "<field>"], "value": <ValueExpr> }
    ]
  }
}
```

- `creates` (optional): Allocates a new entity instance, binding it to `assign` variable.
- `sets` (optional, default `[]`): Field mutations on entity instances.
  - `target`: Two-element array `[variable_name, field_name]`. Variables: `"actor"` for the acting entity, or the `assign` name from `creates`.
  - `value`: A literal (`"private"`, `true`, `42`) or a field reference `["field", "<var>", "<field>"]`.

**Example:**
```json
"create_document": {
  "creates": { "entity": "Document", "assign": "doc" },
  "sets": [
    { "target": ["doc", "owner_id"], "value": ["field", "actor", "id"] },
    { "target": ["doc", "visibility"], "value": "private" },
    { "target": ["doc", "deleted"], "value": false }
  ]
}
```

---

## Section 6: Properties

Invariants and temporal rules that must hold.

```json
"properties": {
  "<property_name>": {
    "type": "invariant" | "temporal",
    "predicate": <Expr>,
    "rule": <TemporalRule>,
    "description": "<human-readable>"
  }
}
```

- **Invariant**: Uses `predicate`. Checked after every state mutation. Must always be true.
- **Temporal**: Uses `rule`. Checked against action traces.

### Temporal Rule Syntax

```json
["before", { "tag": "mutating" }, <Expr>]
["after", "<action>", ["never", "<action>", { "same": "entity" }]]
```

- `["before", trigger, condition]`: Before any action matching `trigger`, `condition` must hold.
- `["after", action, consequence]`: After `action`, `consequence` must hold for all future states.
- `["never", action, scope]`: The given action must never occur (within scope).

---

## Section 7: Generators

Named setup sequences that produce specific initial states.

```json
"generators": {
  "<generator_name>": {
    "description": "<optional>",
    "sequence": [
      { "action": "<action_name>", "with": { ... } }
    ],
    "postcondition": <Expr>
  }
}
```

- `sequence` (required): Ordered list of actions to execute.
- `with` (optional per step): Parameter bindings for the action.
- `postcondition` (optional): Expression that must be true after the sequence completes.

---

## Section 8: Exploration

Configuration for the fuzzing engine's exploration strategy.

```json
"exploration": {
  "weights": {
    "scope": "per_alt_branch_and_model_state",
    "initial": "from_protocol",
    "decay": "per_epoch"
  },
  "directives_allowed": [
    { "type": "<directive_type>", "description": "<optional>" }
  ],
  "adaptation_signals": [
    { "signal": "<signal_type>", "description": "<optional>" }
  ],
  "strategy": {
    "initial": "pseudo_random_traversal",
    "fallback": "targeted_on_violation"
  },
  "epoch_size": <u32>,
  "coverage_floor_threshold": <f64>,
  "concurrency": {
    "mode": "deterministic_interleaving",
    "threads": <u32>
  }
}
```

### Directive Types
`adjust_weight`, `force`, `skip`, `loop_limit`, `swap_observer`

### Signal Types
`coverage_delta`, `property_violation`, `discrepancy`, `crash`, `timeout`, `guard_failure`, `coverage_plateau`

### Recommended Defaults
- `epoch_size`: 100
- `coverage_floor_threshold`: 0.05
- `threads`: 4

---

## Section 9: Inputs

Input space definition for constraint-based test vector generation.

```json
"inputs": {
  "domains": {
    "<domain_name>": <DomainDef>
  },
  "constraints": [
    { "name": "<constraint_name>", "rule": <Expr> }
  ],
  "coverage": {
    "targets": [<CoverageTarget>, ...],
    "seed": <u64>,
    "reproducible": <bool>
  }
}
```

### DomainDef Types

| Type | Schema |
|------|--------|
| `enum` | `{ "type": "enum", "values": ["a", "b", "c"] }` |
| `bool` | `{ "type": "bool" }` |
| `int` | `{ "type": "int", "min": <i64>, "max": <i64> }` |

### CoverageTarget Types

```json
{ "type": "all_pairs", "over": ["domain1", "domain2"] }
{ "type": "each_transition", "machine": "<protocol_name>" }
{ "type": "boundary", "domain": "<domain_name>", "values": [1, 2, 8] }
```

### Constraints
Rules over domain variables. Expressed as `Expr` using domain names as variables.

---

## Section 10: Bindings

Maps abstract actions to concrete DUT (Device Under Test) implementations.

```json
"bindings": {
  "runtime": "wasm",
  "entry": "<wasm_module_path>",
  "actions": {
    "<action_name>": {
      "function": "<exported_function_name>",
      "args": ["<arg1>", "<arg2>"],
      "returns": { "type": "<return_type>" },
      "mutates": <bool>,
      "idempotent": <bool>,
      "reads": ["<EntityName>"],
      "writes": ["<EntityName>"]
    }
  },
  "event_hooks": {
    "mode": "function_intercept",
    "observe": ["<action_name>", ...],
    "capture": ["args", "return_value", "side_effects"]
  }
}
```

---

## Expression Language

Expressions are JSON arrays with an operator tag as the first element.

### Literals
- Boolean: `true`, `false`
- Integer: `42`, `-1`
- String: `"hello"`

### Field Access
```json
["field", "<entity_var>", "<field_name>"]
```
- `"self"` refers to the entity being refined.
- `"actor"` refers to the acting entity.

### Comparison Operators (binary, exactly 2 args)
```json
["eq", <expr>, <expr>]
["neq", <expr>, <expr>]
["lt", <expr>, <expr>]
["lte", <expr>, <expr>]
["gt", <expr>, <expr>]
["gte", <expr>, <expr>]
```

### Logical Operators
```json
["and", <expr>, <expr>, ...]    // 1+ arguments
["or", <expr>, <expr>, ...]     // 1+ arguments
["not", <expr>]                 // exactly 1 argument
["implies", <expr>, <expr>]     // exactly 2 arguments
```

### Quantifiers
```json
["forall", "<var>", "<EntityName>", <body_expr>]
["exists", "<var>", "<EntityName>", <body_expr>]
```

### Function Calls
```json
["derived", "<function_name>", "<arg1>", "<arg2>"]
["observer", "<function_name>", "<arg1>", "<arg2>"]
```

### Refinement Test
```json
["is", "<entity_var>", "<RefinementName>", { "param": "value" }]
```
- The optional params object binds refinement parameters.

---

## Validation Rules

The FresnelFir compiler enforces these structural rules:

1. Every refinement's `base` must reference an existing entity.
2. Every `Call` action in a protocol must have a matching entry in both `effects` and `bindings.actions`.
3. Every `Ref` protocol must reference an existing protocol name.
4. No `Alt` block may have all branches with `weight: 0`.
5. Every `Repeat` must have `min <= max`.
6. Expression nesting depth must not exceed 64 levels.
7. Unary operators (`not`) require exactly 1 argument.
8. Binary operators (`eq`, `neq`, `lt`, `lte`, `gt`, `gte`, `implies`) require exactly 2 arguments.
9. Variadic operators (`and`, `or`) require at least 1 argument.

---

## MCP Tool Integration

After constructing the IR JSON, use these MCP tools:

1. `fresnel_fir_compile` — Validate and compile the IR. Returns `campaign_id` + `budget`.
2. `fresnel_fir_fuzz_start` — Start fuzzing against compiled spec.
3. `fresnel_fir_fuzz_status` — Poll progress (state, iterations, coverage, findings).
4. `fresnel_fir_findings` — Get findings, optionally incremental via `since_seqno`.
5. `fresnel_fir_coverage` — Get coverage targets with hit/pending/unreachable status.
6. `fresnel_fir_abort` — Abort a running campaign.
7. `fresnel_fir_analytics` — Get campaign analytics (coverage curves, finding rates).
8. `fresnel_fir_status` — Get engine-wide status.
