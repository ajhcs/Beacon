# CLAUDE.md

## Project Overview

FresnelFir is a self-improving verification harness for AI-assisted software development. It compiles declarative IR specifications into state machines and refinement types, then uses property-based fuzzing to verify AI-generated code running in a WASM sandbox. The system enforces formal constraints through directed exploration, adaptation, and runtime containment.

## Development Commands

**Build the workspace:**
```bash
cargo build
```

**Run tests (all crates):**
```bash
cargo test
```

**Test specific crate:**
```bash
cargo test -p fresnel-fir-ir
cargo test -p fresnel-fir-compiler
cargo test -p fresnel-fir-model
# etc.
```

**Build release binary:**
```bash
cargo build --release
```

**Check code without building:**
```bash
cargo check
```

## Architecture

**Cargo workspace** with progressive layers. Each layer is independently testable. All crates live in `crates/` subdirectory:

- **fresnel-fir-ir** — Core IR types and expression AST. JSON-based declarative specification (entities, refinements, functions, protocols, effects, properties, generators, exploration config, inputs, bindings).

- **fresnel-fir-compiler** — Validates and compiles the IR. Transforms into:
  - Expression predicates (type-checked, nesting depth ≤64)
  - Protocol state machines (sequences, alternatives, loops, calls, references)
  - Refinement type constraints
  - Execution graphs for traversal

- **fresnel-fir-model** — Model state representation and mutation. Tracks entity instances, applies effect deltas, evaluates refinement predicates against current state. Feeds into constraint checking and symbolic analysis.

- **fresnel-fir-sandbox** — WASM runtime using Wasmtime. Loads DUT (Device Under Test) modules, manages execution isolation, injects stimuli from traversal engine, intercepts function calls, captures return values and side effects.

- **fresnel-fir-explore** — Fuzzing and traversal engine. Executes protocol state machines, manages exploration weights, tracks coverage, applies adaptation directives. Uses constraint solving (varisat or z3) for test vector generation. Rayon for parallel traversal.

- **fresnel-fir-vif** — Verification infrastructure framework. MCP server (Tokio-based), JSON marshalling, campaign management, findings accumulation, analytics.

- **fresnel-fir-core** — Top-level binary entry point. Coordinates all layers: IR compilation → model setup → WASM loading → fuzzing execution → findings reporting.

**Key directories:**
- `crates/` — All Rust crates (source + tests)
- `docs/plans/` — Implementation plan (layer-by-layer breakdown)
- `docs/beacon-ir-schema.md` — Complete IR specification (human-readable reference)
- `.claude/plugins/fresnel-fir/` — Claude Code plugin skills and hooks (Socratic workflow, smoke checks)

## Key Configuration

**Cargo.toml (workspace root):**
- Resolver: `2`
- Edition: `2021`
- License: `MIT`
- Workspace members: 7 crates (ir, compiler, model, sandbox, explore, vif, core)
- Shared dependencies: serde, serde_json, thiserror, wasmtime, wat, varisat, rayon, crossbeam, rand, tokio

**.gitignore:**
- `/target` (build artifacts only)

**FresnelFir IR Schema** (`docs/beacon-ir-schema.md`):
- 9 required top-level sections: entities, refinements, functions, protocols, effects, properties, generators, exploration, inputs, bindings
- All sections required (use empty objects/arrays if unused)
- Expression language: JSON arrays with operator tags (no string parsing)
- Validation rules: strict schema enforcement, nesting depth ≤64, all references validated

**Protocol Specification:**
- `protocols` define valid operation sequences (seq, alt, repeat, call, ref)
- `effects` define state mutations per action (creates, sets fields)
- `properties` enforce invariants and temporal rules

**Bindings:**
- `runtime: "wasm"` (Wasmtime-based)
- `entry` specifies WASM module path
- `actions` map abstract actions to DUT functions (args, returns, mutates, idempotent flags, read/write sets)

## Conventions

**Module organization:**
- Public interfaces in `pub mod` declarations (lib.rs)
- Type definitions grouped by concern (expr, types, validate, etc.)
- Tests colocated in `tests/` subdirectory per crate
- Error types use `thiserror` crate

**Expression deserialization:**
- Literals (bool, int, string) deserialize directly
- Array expressions: `[op, arg1, arg2, ...]` with known operator tags (eq, neq, and, or, not, implies, field, forall, exists, derived, observer, is)
- Custom serde Deserializer handles JSON array format without ambiguity
- Expressions are immutable once compiled

**State machine patterns:**
- Protocols define deterministic paths through refinement types
- NDA (nested data abstraction) traversal explores protocol branches
- Coverage measured by branch hits and state reachability
- Adaptation only modifies exploration strategy, never relaxes constraints

**Error handling:**
- Use `thiserror::Error` for all error types
- Provide context in error messages (which IR section, which validation rule failed)
- Propagate errors up (no silent failures)

**Testing:**
- Test IR parsing with realistic JSON examples
- Test compiler validation against both valid and invalid IR specs
- Test model state mutations against effect semantics
- Test WASM sandbox isolation and function interception
- Use deterministic RNG seeds for reproducible fuzzing tests

**Rust idiom compliance:**
- Edition 2021 (async/await, const generics, etc.)
- No unsafe code unless absolutely necessary (sandbox boundary calls)
- Generic code for parameterized refinements and exploration strategies
- Owned types (Vec, String) for mutable collections; references for read-only access
