# Beacon Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a self-improving verification harness for AI-assisted software development, starting from the Declarative IR compiler and progressively adding model state, WASM sandbox, solver, traversal, and adaptation.

**Architecture:** Cargo workspace with internal crates (`beacon-ir`, `beacon-compiler`, `beacon-model`, `beacon-explore`, `beacon-sandbox`, `beacon-vif`, `beacon-core`). Single native binary. Progressive layers where each is independently testable. See `docs/plans/2026-02-17-beacon-harness-design.md` for full design.

**Tech Stack:** Rust (2021 edition), serde/serde_json, wasmtime (Layer 2), z3-sys or varisat (Layer 3), rayon (Layer 4), tokio (MCP server), crossbeam (channels).

---

## Layer 0: Foundation + IR Compiler

### Task 0.1: Workspace Scaffold

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/beacon-ir/Cargo.toml`
- Create: `crates/beacon-ir/src/lib.rs`
- Create: `crates/beacon-compiler/Cargo.toml`
- Create: `crates/beacon-compiler/src/lib.rs`

**Step 1: Create workspace root Cargo.toml**

```toml
[workspace]
resolver = "2"
members = [
    "crates/beacon-ir",
    "crates/beacon-compiler",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
```

**Step 2: Create beacon-ir crate**

```toml
# crates/beacon-ir/Cargo.toml
[package]
name = "beacon-ir"
version.workspace = true
edition.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
```

```rust
// crates/beacon-ir/src/lib.rs
pub mod expr;
pub mod types;
pub mod parse;
```

**Step 3: Create beacon-compiler crate**

```toml
# crates/beacon-compiler/Cargo.toml
[package]
name = "beacon-compiler"
version.workspace = true
edition.workspace = true

[dependencies]
beacon-ir = { path = "../beacon-ir" }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
```

```rust
// crates/beacon-compiler/src/lib.rs
pub mod validate;
pub mod predicate;
pub mod protocol;
pub mod graph;
```

**Step 4: Verify workspace builds**

Run: `cargo build`
Expected: Compiles with no errors.

**Step 5: Commit**

```bash
git add Cargo.toml crates/
git commit -m "scaffold: workspace with beacon-ir and beacon-compiler crates"
```

---

### Task 0.2: Expression AST Types (`beacon-ir/src/expr.rs`)

The structured predicate language — JSON AST expressions, no string parsing.

**Files:**
- Create: `crates/beacon-ir/src/expr.rs`
- Test: `crates/beacon-ir/tests/expr_tests.rs`

**Step 1: Write the failing test**

```rust
// crates/beacon-ir/tests/expr_tests.rs
use beacon_ir::expr::Expr;

#[test]
fn test_parse_literal_bool() {
    let json = serde_json::json!(true);
    let expr: Expr = serde_json::from_value(json).unwrap();
    assert!(matches!(expr, Expr::Literal(beacon_ir::expr::Literal::Bool(true))));
}

#[test]
fn test_parse_literal_string() {
    let json = serde_json::json!("public");
    let expr: Expr = serde_json::from_value(json).unwrap();
    assert!(matches!(expr, Expr::Literal(beacon_ir::expr::Literal::String(s)) if s == "public"));
}

#[test]
fn test_parse_eq_expression() {
    let json = serde_json::json!(["eq", ["field", "self", "authenticated"], true]);
    let expr: Expr = serde_json::from_value(json).unwrap();
    assert!(matches!(expr, Expr::Op { .. }));
}

#[test]
fn test_parse_nested_and_or() {
    let json = serde_json::json!(["or",
        ["eq", ["field", "self", "visibility"], "public"],
        ["and",
            ["eq", ["field", "self", "visibility"], "shared"],
            ["neq", ["field", "actor", "role"], "guest"]
        ]
    ]);
    let expr: Expr = serde_json::from_value(json).unwrap();
    assert!(matches!(expr, Expr::Op { .. }));
}

#[test]
fn test_parse_field_access() {
    let json = serde_json::json!(["field", "self", "owner_id"]);
    let expr: Expr = serde_json::from_value(json).unwrap();
    assert!(matches!(expr, Expr::Field { .. }));
}

#[test]
fn test_parse_forall_quantifier() {
    let json = serde_json::json!(["forall", "d", "Document",
        ["not", ["derived", "canAccess", "u", "d"]]
    ]);
    let expr: Expr = serde_json::from_value(json).unwrap();
    assert!(matches!(expr, Expr::Quantifier { .. }));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p beacon-ir`
Expected: Compilation error — `Expr` type doesn't exist yet.

**Step 3: Implement expression types**

```rust
// crates/beacon-ir/src/expr.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Expr {
    Literal(Literal),
    Field {
        // Deserialized from ["field", "entity", "field_name"]
        // Uses custom deserializer
        entity: String,
        field: String,
    },
    Op {
        op: OpKind,
        args: Vec<Expr>,
    },
    Quantifier {
        kind: QuantifierKind,
        var: String,
        domain: String,
        body: Box<Expr>,
    },
    FnCall {
        classification: FnClassification,
        name: String,
        args: Vec<String>,
    },
    Is {
        entity: String,
        refinement: String,
        params: std::collections::HashMap<String, String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Literal {
    Bool(bool),
    Int(i64),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpKind {
    Eq,
    Neq,
    And,
    Or,
    Not,
    Implies,
    Lt,
    Lte,
    Gt,
    Gte,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantifierKind {
    Forall,
    Exists,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FnClassification {
    Derived,
    Observer,
}
```

Note: The JSON array format `["eq", arg1, arg2]` requires a custom serde Deserializer. Implement `impl<'de> Deserialize<'de> for Expr` that:
1. Checks if value is a bool/number/string -> `Literal`
2. Checks if value is an array starting with a known operator string -> `Op`
3. Checks if array starts with "field" -> `Field`
4. Checks if array starts with "forall"/"exists" -> `Quantifier`
5. Checks if array starts with "derived"/"observer" -> `FnCall`
6. Checks if array starts with "is" -> `Is`

The custom deserializer is the most complex part of this task. It must handle the JSON array format without ambiguity.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p beacon-ir`
Expected: All 6 tests pass.

**Step 5: Commit**

```bash
git add crates/beacon-ir/src/expr.rs crates/beacon-ir/tests/
git commit -m "feat(ir): expression AST types with JSON array deserialization"
```

---

### Task 0.3: Core IR Types (`beacon-ir/src/types.rs`)

All 9 IR sections as Rust types.

**Files:**
- Create: `crates/beacon-ir/src/types.rs`
- Test: `crates/beacon-ir/tests/types_tests.rs`

**Step 1: Write the failing test**

```rust
// crates/beacon-ir/tests/types_tests.rs
use beacon_ir::types::BeaconIR;

#[test]
fn test_parse_minimal_ir() {
    let json = serde_json::json!({
        "entities": {
            "User": {
                "fields": {
                    "id": { "type": "string", "format": "uuid" },
                    "role": { "type": "enum", "values": ["admin", "guest"] }
                }
            }
        },
        "refinements": {},
        "functions": {},
        "protocols": {},
        "effects": {},
        "properties": {},
        "generators": {},
        "exploration": {
            "weights": { "scope": "per_alt_branch_and_model_state", "initial": "from_protocol", "decay": "per_epoch" },
            "directives_allowed": [],
            "adaptation_signals": [],
            "strategy": { "initial": "pseudo_random_traversal", "fallback": "targeted_on_violation" },
            "epoch_size": 100,
            "coverage_floor_threshold": 0.05,
            "concurrency": { "mode": "deterministic_interleaving", "threads": 4 }
        },
        "inputs": {
            "domains": {},
            "constraints": [],
            "coverage": { "targets": [], "seed": 42, "reproducible": true }
        },
        "bindings": {
            "runtime": "wasm",
            "entry": "main.wasm",
            "actions": {},
            "event_hooks": { "mode": "function_intercept", "observe": [], "capture": [] }
        }
    });
    let ir: BeaconIR = serde_json::from_value(json).unwrap();
    assert_eq!(ir.entities.len(), 1);
    assert!(ir.entities.contains_key("User"));
}

#[test]
fn test_parse_entity_fields() {
    let json = serde_json::json!({
        "type": "enum",
        "values": ["admin", "member", "guest"]
    });
    let field: beacon_ir::types::FieldDef = serde_json::from_value(json).unwrap();
    assert!(matches!(field.field_type, beacon_ir::types::FieldType::Enum { .. }));
}

#[test]
fn test_parse_protocol_with_grammar_constructs() {
    let json = serde_json::json!({
        "root": {
            "type": "seq",
            "children": [
                { "type": "call", "action": "create" },
                {
                    "type": "alt",
                    "branches": [
                        { "id": "a", "weight": 50, "body": { "type": "call", "action": "read" } },
                        { "id": "b", "weight": 50, "body": { "type": "call", "action": "delete" } }
                    ]
                }
            ]
        }
    });
    let proto: beacon_ir::types::Protocol = serde_json::from_value(json).unwrap();
    assert!(matches!(proto.root, beacon_ir::types::ProtocolNode::Seq { .. }));
}

#[test]
fn test_parse_effect() {
    let json = serde_json::json!({
        "creates": { "entity": "Document", "assign": "doc" },
        "sets": [
            { "target": ["doc", "visibility"], "value": "private" }
        ]
    });
    let effect: beacon_ir::types::Effect = serde_json::from_value(json).unwrap();
    assert!(effect.creates.is_some());
    assert_eq!(effect.sets.len(), 1);
}

#[test]
fn test_parse_action_binding() {
    let json = serde_json::json!({
        "function": "create_document",
        "args": ["actor_id"],
        "returns": { "type": "Document" },
        "mutates": true,
        "idempotent": false,
        "reads": [],
        "writes": ["Document"]
    });
    let binding: beacon_ir::types::ActionBinding = serde_json::from_value(json).unwrap();
    assert!(binding.mutates);
    assert!(!binding.idempotent);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p beacon-ir`
Expected: Compilation error — types don't exist yet.

**Step 3: Implement all IR types**

Implement in `crates/beacon-ir/src/types.rs`:

- `BeaconIR` — top-level struct with all 9 sections
- `Entity`, `FieldDef`, `FieldType` — entity definitions
- `Refinement`, `FunctionDef` — refinement types + classified functions
- `Protocol`, `ProtocolNode` (enum: `Seq`, `Alt`, `Repeat`, `Call`, `Ref`), `AltBranch` — protocol grammar
- `Effect`, `EffectSet`, `CreateEffect` — action effects
- `Property`, `PropertyType` — invariants + temporal rules
- `Generator`, `GeneratorStep` — setup sequences
- `ExplorationConfig`, `WeightConfig`, `DirectiveConfig`, `AdaptationSignal`, `StrategyConfig`, `ConcurrencyConfig` — exploration
- `InputSpace`, `Domain`, `DomainType`, `InputConstraint`, `CoverageConfig`, `CoverageTarget` — inputs
- `Bindings`, `ActionBinding`, `EventHooks` — DUT bindings

All types derive `Debug, Clone, Serialize, Deserialize`. Use `serde(rename_all = "snake_case")` where appropriate.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p beacon-ir`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add crates/beacon-ir/
git commit -m "feat(ir): core IR types for all 9 Declarative IR sections"
```

---

### Task 0.4: IR JSON Parsing (`beacon-ir/src/parse.rs`)

Parse a complete IR JSON document into the typed AST.

**Files:**
- Create: `crates/beacon-ir/src/parse.rs`
- Create: `crates/beacon-ir/tests/fixtures/document_lifecycle.json`
- Test: `crates/beacon-ir/tests/parse_tests.rs`

**Step 1: Create the test fixture**

Create `crates/beacon-ir/tests/fixtures/document_lifecycle.json` containing the full document lifecycle IR example from the design document (all 9 sections populated with the User/Document example).

**Step 2: Write the failing test**

```rust
// crates/beacon-ir/tests/parse_tests.rs
use beacon_ir::parse::parse_ir;

#[test]
fn test_parse_full_ir_from_file() {
    let json_str = include_str!("fixtures/document_lifecycle.json");
    let ir = parse_ir(json_str).unwrap();
    assert_eq!(ir.entities.len(), 2); // User, Document
    assert!(ir.entities.contains_key("User"));
    assert!(ir.entities.contains_key("Document"));
    assert!(!ir.refinements.is_empty());
    assert!(!ir.protocols.is_empty());
    assert!(!ir.effects.is_empty());
    assert!(!ir.properties.is_empty());
}

#[test]
fn test_parse_invalid_json() {
    let result = parse_ir("not json at all");
    assert!(result.is_err());
}

#[test]
fn test_parse_empty_sections() {
    let json = r#"{
        "entities": {},
        "refinements": {},
        "functions": {},
        "protocols": {},
        "effects": {},
        "properties": {},
        "generators": {},
        "exploration": {
            "weights": { "scope": "per_alt_branch_and_model_state", "initial": "from_protocol", "decay": "per_epoch" },
            "directives_allowed": [],
            "adaptation_signals": [],
            "strategy": { "initial": "pseudo_random_traversal", "fallback": "targeted_on_violation" },
            "epoch_size": 100,
            "coverage_floor_threshold": 0.05,
            "concurrency": { "mode": "deterministic_interleaving", "threads": 4 }
        },
        "inputs": {
            "domains": {},
            "constraints": [],
            "coverage": { "targets": [], "seed": 42, "reproducible": true }
        },
        "bindings": {
            "runtime": "wasm",
            "entry": "main.wasm",
            "actions": {},
            "event_hooks": { "mode": "function_intercept", "observe": [], "capture": [] }
        }
    }"#;
    let ir = parse_ir(json).unwrap();
    assert!(ir.entities.is_empty());
}
```

**Step 3: Implement parse function**

```rust
// crates/beacon-ir/src/parse.rs
use crate::types::BeaconIR;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn parse_ir(json: &str) -> Result<BeaconIR, ParseError> {
    Ok(serde_json::from_str(json)?)
}
```

**Step 4: Run tests**

Run: `cargo test -p beacon-ir`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add crates/beacon-ir/
git commit -m "feat(ir): JSON parsing with full IR fixture test"
```

---

### Task 0.5: Structural Validation (`beacon-compiler/src/validate.rs`)

Validate that a parsed IR is internally consistent.

**Files:**
- Create: `crates/beacon-compiler/src/validate.rs`
- Test: `crates/beacon-compiler/tests/validate_tests.rs`

**Step 1: Write failing tests**

```rust
// crates/beacon-compiler/tests/validate_tests.rs
use beacon_compiler::validate::{validate_ir, ValidationError};
use beacon_ir::parse::parse_ir;

#[test]
fn test_valid_ir_passes() {
    let json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let ir = parse_ir(json).unwrap();
    let result = validate_ir(&ir);
    assert!(result.is_ok());
}

#[test]
fn test_dangling_entity_ref_in_refinement() {
    // Refinement references entity "Ghost" which doesn't exist
    let json = r#"{
        "entities": {},
        "refinements": {
            "BadRef": {
                "base": "Ghost",
                "predicate": true
            }
        },
        "functions": {},
        "protocols": {},
        "effects": {},
        "properties": {},
        "generators": {},
        "exploration": { "weights": { "scope": "per_alt_branch_and_model_state", "initial": "from_protocol", "decay": "per_epoch" }, "directives_allowed": [], "adaptation_signals": [], "strategy": { "initial": "pseudo_random_traversal", "fallback": "targeted_on_violation" }, "epoch_size": 100, "coverage_floor_threshold": 0.05, "concurrency": { "mode": "deterministic_interleaving", "threads": 4 } },
        "inputs": { "domains": {}, "constraints": [], "coverage": { "targets": [], "seed": 42, "reproducible": true } },
        "bindings": { "runtime": "wasm", "entry": "main.wasm", "actions": {}, "event_hooks": { "mode": "function_intercept", "observe": [], "capture": [] } }
    }"#;
    let ir = parse_ir(json).unwrap();
    let result = validate_ir(&ir);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| matches!(e, ValidationError::DanglingEntityRef { .. })));
}

#[test]
fn test_action_without_effect() {
    // Protocol references action "publish" but no effect defined for it
    // (Create test with a protocol that calls "publish" and empty effects map)
    // Expected: ValidationError::MissingEffect
}

#[test]
fn test_action_without_binding() {
    // Protocol references action but no binding defined
    // Expected: ValidationError::MissingBinding
}

#[test]
fn test_dangling_protocol_ref() {
    // Protocol uses ref to "idle" but "idle" protocol doesn't exist
    // Expected: ValidationError::DanglingProtocolRef
}

#[test]
fn test_alt_all_zero_weights() {
    // Alt block where all branches have weight 0
    // Expected: ValidationError::AllZeroWeights
}

#[test]
fn test_repeat_min_exceeds_max() {
    // Repeat with min: 10, max: 5
    // Expected: ValidationError::InvalidRepeatBounds
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p beacon-compiler`
Expected: Compilation error.

**Step 3: Implement validation**

```rust
// crates/beacon-compiler/src/validate.rs
use beacon_ir::types::BeaconIR;

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Dangling entity reference: refinement '{refinement}' references entity '{entity}' which doesn't exist")]
    DanglingEntityRef { refinement: String, entity: String },

    #[error("Missing effect: action '{action}' used in protocol but no effect defined")]
    MissingEffect { action: String },

    #[error("Missing binding: action '{action}' used in protocol but no binding defined")]
    MissingBinding { action: String },

    #[error("Dangling protocol ref: '{from}' references protocol '{target}' which doesn't exist")]
    DanglingProtocolRef { from: String, target: String },

    #[error("All zero weights in alt block at '{location}'")]
    AllZeroWeights { location: String },

    #[error("Invalid repeat bounds at '{location}': min ({min}) > max ({max})")]
    InvalidRepeatBounds { location: String, min: u32, max: u32 },
}

pub fn validate_ir(ir: &BeaconIR) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    validate_entity_refs(ir, &mut errors);
    validate_protocol_actions(ir, &mut errors);
    validate_protocol_refs(ir, &mut errors);
    validate_protocol_structure(ir, &mut errors);
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
```

Implement each `validate_*` function to walk the IR and collect errors. Walk protocol nodes recursively to find all `Call` and `Ref` nodes, all `Alt` blocks (check weights), all `Repeat` nodes (check bounds).

**Step 4: Run tests**

Run: `cargo test -p beacon-compiler`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add crates/beacon-compiler/
git commit -m "feat(compiler): structural validation for IR consistency"
```

---

### Task 0.6: Predicate Compilation (`beacon-compiler/src/predicate.rs`)

Compile expression ASTs into an evaluable internal representation.

**Files:**
- Create: `crates/beacon-compiler/src/predicate.rs`
- Test: `crates/beacon-compiler/tests/predicate_tests.rs`

**Step 1: Write failing tests**

Test that expressions can be type-checked against entity definitions (field references resolve to declared fields with correct types). Test that compiled predicates can be evaluated against a simple value environment.

**Step 2: Implement**

- `CompiledExpr` enum — mirrors `Expr` but with resolved type information
- `compile_expr(expr: &Expr, ctx: &TypeContext) -> Result<CompiledExpr, CompileError>`
- `TypeContext` holds entity/field/refinement type information from the IR
- `eval_expr(expr: &CompiledExpr, env: &ValueEnv) -> Result<Value, EvalError>` for runtime evaluation
- Type checking: verify field references resolve, operator argument types match, quantifier domains are declared entities

**Step 3: Run tests, commit**

---

### Task 0.7: Protocol Compilation to NDA Graph (`beacon-compiler/src/protocol.rs`, `beacon-compiler/src/graph.rs`)

Compile grammar constructs into traversable NDA graphs.

**Files:**
- Create: `crates/beacon-compiler/src/protocol.rs`
- Create: `crates/beacon-compiler/src/graph.rs`
- Test: `crates/beacon-compiler/tests/protocol_tests.rs`

**Step 1: Write failing tests**

Test that a simple `seq` of two `call` nodes produces a graph with 2 terminal nodes in sequence. Test that `alt` produces branch points with correct weights. Test that `repeat` produces loop structures. Test that `ref` inlines the referenced protocol.

**Step 2: Implement graph types**

```rust
// crates/beacon-compiler/src/graph.rs
pub type NodeId = u32;

pub enum GraphNode {
    Terminal { action: String, guard: Option<CompiledExpr> },
    Branch { alternatives: Vec<BranchEdge> },
    LoopEntry { body_start: NodeId, min: u32, max: u32 },
    LoopExit,
    Start,
    End,
}

pub struct BranchEdge {
    pub id: String,
    pub weight: f64,
    pub target: NodeId,
    pub guard: Option<CompiledExpr>,
}

pub struct NdaGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<(NodeId, NodeId)>,
    pub entry: NodeId,
    pub exit: NodeId,
}
```

**Step 3: Implement protocol compiler**

`compile_protocol(proto: &Protocol, ctx: &TypeContext) -> Result<NdaGraph, CompileError>`

Recursively walk `ProtocolNode` tree:
- `Seq` -> chain nodes sequentially
- `Alt` -> create Branch node with weighted edges to each branch's subgraph
- `Repeat` -> create LoopEntry/LoopExit bracketing the body subgraph
- `Call` -> create Terminal node
- `Ref` -> recursively compile referenced protocol and inline

**Step 4: Run tests, commit**

---

### Task 0.8: Integration — Full Compilation Pipeline

**Files:**
- Create: `crates/beacon-compiler/src/compile.rs`
- Test: `crates/beacon-compiler/tests/integration_tests.rs`

**Step 1: Write failing test**

```rust
#[test]
fn test_full_compilation_pipeline() {
    let json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let ir = beacon_ir::parse::parse_ir(json).unwrap();
    let result = beacon_compiler::compile(&ir);
    assert!(result.is_ok());
    let compiled = result.unwrap();
    assert!(!compiled.graphs.is_empty());
    assert!(!compiled.predicates.is_empty());
}
```

**Step 2: Implement top-level compile function**

```rust
// crates/beacon-compiler/src/compile.rs
pub struct CompiledIR {
    pub graphs: HashMap<String, NdaGraph>,
    pub predicates: HashMap<String, CompiledExpr>,
    pub type_context: TypeContext,
    pub ir_hash: [u8; 32],
}

pub fn compile(ir: &BeaconIR) -> Result<CompiledIR, CompileError> {
    // 1. Validate
    validate_ir(ir).map_err(CompileError::Validation)?;
    // 2. Build type context
    let ctx = TypeContext::from_ir(ir);
    // 3. Compile predicates (refinements, properties, guards)
    // 4. Compile protocols into NDA graphs
    // 5. Hash the IR for reproducibility
    // 6. Return CompiledIR
}
```

**Step 3: Run tests, commit**

---

## Layer 1: Model State + Property Checking

### Task 1.1: Add beacon-model crate to workspace

Add `crates/beacon-model/` with CoW model state types (`ModelState`, `Entity`, `EntityInstance`), `fork()`, `snapshot()`, `rollback()`.

### Task 1.2: Effect application

Implement `apply_effect(state: &mut ModelState, effect: &Effect, args: &ValueEnv) -> Result<(), EffectError>`. Test that `create` allocates new entity, `sets` update fields.

### Task 1.3: Derived function evaluation

Implement `eval_derived(state: &ModelState, func: &CompiledExpr, args: &[Value]) -> Result<Value, EvalError>`. Test that `canAccess` computes correctly from model state.

### Task 1.4: Invariant checking

Implement `check_invariants(state: &ModelState, properties: &[CompiledProperty]) -> Vec<Violation>`. Test with ownership_isolation invariant — should pass with correct state, fail with violating state.

### Task 1.5: Temporal property checking

Implement trace-based temporal checking: `check_temporal(trace: &[TraceEntry], rules: &[TemporalRule]) -> Vec<Violation>`. Test `auth_before_mutation` and `delete_is_permanent`.

### Task 1.6: Model simulation (no DUT)

Walk NDA graph using model state + effects only. Produce coverage report. Detect spec contradictions.

---

## Layer 2: WASM Sandbox + Verification Adapter

### Task 2.1: Add beacon-sandbox and beacon-vif crates

Add wasmtime dependency. Create sandbox configuration types (memory limits, fuel, isolation).

### Task 2.2: WASM loading and isolation

Load a WASM module, configure wasmtime with no WASI FS/network/clock, fuel metering, memory cap. Test that isolated module cannot access filesystem.

### Task 2.3: Verification adapter generation

From IR bindings, generate the action stubs (serialize args -> call WASM export -> deserialize return) and observer stubs. Test with a simple WASM module that exports matching functions.

### Task 2.4: Interface validation

At DUT load time, verify WASM exports match declared bindings. Test with a module missing an expected export.

### Task 2.5: Snapshot/restore

Implement paired WASM + model state snapshots. Test that restore produces identical state to the snapshot point.

### Task 2.6: Single action execution

Execute one action against loaded DUT: serialize args, call, capture response, apply model effects, compare. First time model truth meets observed behavior.

---

## Layer 3: Solver + Vector Generation

### Task 3.1: Add solver dependency (z3-sys or varisat)

Evaluate Z3 FFI vs varisat. Set up beacon-explore/solver module.

### Task 3.2: Domain encoding

Encode enum/bool/int domains as solver variables. Test round-trip: encode -> solve -> decode.

### Task 3.3: Constraint encoding

Translate IR constraint predicates to solver assertions. Test with simple constraints (guest_never_admin).

### Task 3.4: Basic solve + search

Given domains + constraints, find one satisfying vector. Then find multiple unique vectors. Test uniqueness.

### Task 3.5: Fracture algorithm

Implement input space fracturing by variable. Test that fracturing produces correct subspaces.

### Task 3.6: Parallel solve with abort

Run subspaces in parallel (rayon). Abort UNSAT branches. Test with a mix of SAT and UNSAT subspaces.

### Task 3.7: Coverage-driven generation

Generate vectors targeting specific coverage points. Test all-pairs coverage for a 3-variable domain.

### Task 3.8: Per-stage RNG seeding

Implement stage stacks with seeded RNG. Test reproducibility: same seed produces identical vectors.

### Task 3.9: Vector pool

Lockfree queue of pre-generated vectors, indexed by coverage target. Test concurrent reads from multiple threads.

---

## Layer 4: Traversal Engine + Directed Fuzzing

### Task 4.1: Object stack + strategy stack

Implement the core traversal loop: pop node, delegate to strategy, push children.

### Task 4.2: PseudoRandom strategy

Implement weighted random selection at alt branches, random loop count selection at repeats.

### Task 4.3: State-conditioned weight table

Implement `WeightTable` keyed by `(AltBranchId, AbstractModelStateId)`. Test state-dependent weight lookup.

### Task 4.4: Full action pipeline

Integrate: guard check -> vector selection -> sandbox call -> effect application -> invariant check -> observer query -> comparison -> signal emission.

### Task 4.5: Signal + epoch processing

Implement coordinator with epoch barriers, monotonic signal_seqno assignment, deterministic directive ordering.

### Task 4.6: Directive processing

Implement adjust_weight, force, skip, loop_limit, swap_observer. Test each directive type.

### Task 4.7: Strategy stack (Targeted, Investigation, Force)

Implement strategy push/pop with depth limit. Test plateau -> Force compilation.

### Task 4.8: Replay capsule generation

Capture full reproduction state for each finding. Test single-thread deterministic replay.

### Task 4.9: Concurrent traversal

Multiple traversal threads + coordinator thread + lockfree channels. Test deterministic interleaving mode.

### Task 4.10: Campaign lifecycle

Implement beacon_fuzz_start/status/findings/coverage/reproduce/abort. Integrate with MCP server.

---

## Layer 5: Adaptation + Cross-Campaign Learning

### Task 5.1: Weight decay + normalization

Per-epoch weight decay, boost on findings, normalization to 100.

### Task 5.2: Coverage floor enforcement

Compute reachability mass per uncovered target, boost if below threshold.

### Task 5.3: Timeout two-step response

Shrink fuel envelope -> retry -> bounded skip -> performance finding.

### Task 5.4: Provable unreachability

Static reachability analysis + solver UNSAT -> permanent zero with proof artifact.

### Task 5.5: Cross-campaign memory persistence

Serialize/deserialize CampaignMemory. Replay capsule persistence. Weight decay across campaigns.

### Task 5.6: Invalidation triggers

Track consecutive non-reproductions. Aggressive decay after K failures.

### Task 5.7: Re-regression priority

Campaign start order: replay capsules -> hot regions -> coverage exploration.

---

## Layer 6: Polish + Production Hardening

### Task 6.1: Error message quality pass
### Task 6.2: Campaign analytics / telemetry
### Task 6.3: Resource limits and graceful degradation
### Task 6.4: IR schema documentation for AI consumption
### Task 6.5: Claude Code skills (Socratic workflow + verification loop)
### Task 6.6: Claude Code hooks (smoke check + stop gate)
