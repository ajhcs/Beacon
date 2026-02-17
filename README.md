# FresnelFir

**Automated verification for AI-generated code.**

FresnelFir catches bugs that tests miss. You describe *what your code should do* in a declarative specification, and FresnelFir systematically explores every path through your program to find violations — before your users do.

Built in Rust. Runs your code in a WASM sandbox. Finds real bugs with formal methods, not just the ones you thought to test for.

[![CI](https://github.com/ajhcs/Beacon/actions/workflows/ci.yml/badge.svg)](https://github.com/ajhcs/Beacon/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## What FresnelFir Does (Plain English)

Traditional testing checks specific scenarios you write by hand. Property-based testing generates random inputs but still relies on you knowing what to check. FresnelFir takes a fundamentally different approach:

1. **You describe the rules** — "a deleted document stays deleted," "authentication must happen before mutation," "field values stay within these bounds." You write these as a JSON specification, not code.

2. **FresnelFir explores systematically** — It compiles your rules into a state machine, then walks every reachable path through your program's protocol. It doesn't just throw random inputs at your code; it uses constraint solving to generate inputs that target uncovered branches.

3. **Your code runs in a sandbox** — FresnelFir loads your program as a WebAssembly module inside a secure, fuel-metered sandbox. It can snapshot and restore execution state, replay exact sequences, and catch crashes without bringing down the host.

4. **Discrepancies become findings** — FresnelFir maintains a pure model of what *should* happen alongside the actual behavior of your code. When the model and reality diverge, that's a finding — a concrete, reproducible test case you can debug.

This is the same verification methodology used in hardware design (where shipping a bug means recalling silicon), adapted for software and specifically for the emerging world of AI-assisted code generation, where the volume of machine-written code outpaces human review capacity.

## Why This Exists

AI coding assistants can generate thousands of lines of code per hour. No human team can review that volume with the rigor needed for production systems. FresnelFir closes the gap: the AI writes code, FresnelFir proves it correct (or finds where it isn't), and the AI fixes what's broken — in an automated loop with no human bottleneck.

FresnelFir's core algorithms originate from four US patents developed at **Silicon Forest Research**, which later merged into **Lighthouse Design Automation**. The patents cover NDA (Nested Data Abstraction) graph traversal, the fracture/solve/abort constraint decomposition strategy, and modification directives for adaptive exploration. These techniques were originally designed for hardware verification and have been reimplemented here for software.

## Key Features

- **Declarative IR specification** — Define entities, protocols, effects, properties, and constraints in JSON. No DSL to learn, no compiler to install. Nine required sections give you a complete formal specification.
- **Protocol-driven exploration** — Define valid operation sequences (sequences, alternatives, loops, calls) that get compiled into NDA graphs and systematically traversed.
- **Constraint-based test generation** — Uses SAT solving (Varisat) to generate test vectors that satisfy complex constraints and target uncovered regions of the state space.
- **WASM sandboxing** — Your code runs in Wasmtime with fuel metering, memory limits, and snapshot/restore. Crashes are contained, never propagated.
- **Model-vs-reality verification** — Pure model functions define expected behavior; observers query actual DUT state. Discrepancies are findings with full reproduction context.
- **Adaptive exploration** — Epoch-based signal processing adjusts traversal weights in real time: boosting underexplored branches, decaying overexplored ones, proving unreachable paths.
- **Temporal property checking** — Express ordering constraints ("auth before mutation," "delete is permanent") that are checked against execution traces.
- **MCP server integration** — AI coding agents interact with FresnelFir through a Model Context Protocol server, enabling fully automated verify-fix loops.
- **Deterministic replay** — Every finding includes a replay capsule with the exact seed, inputs, and state needed to reproduce it on a single thread.
- **Cross-campaign learning** — Hot regions, regression capsules, and weight histories persist across verification campaigns for cumulative coverage improvement.

## Architecture

FresnelFir is a Cargo workspace of seven composable crates, organized in progressive layers where each layer builds on the previous and is independently testable:

```
┌─────────────────────────────────────────────────────┐
│                   fresnel-fir-core                   │  Campaign management, analytics,
│                                                     │  MCP server, resource limits
├────────────────────────┬────────────────────────────┤
│   fresnel-fir-explore  │     fresnel-fir-vif        │  Traversal engine, solver,
│                        │                            │  verification adapter
├────────────────────────┼────────────────────────────┤
│   fresnel-fir-model    │   fresnel-fir-sandbox      │  Model state, effects,
│                        │                            │  WASM containment
├────────────────────────┴────────────────────────────┤
│              fresnel-fir-compiler                    │  IR validation, predicate
│                                                     │  compilation, NDA graphs
├─────────────────────────────────────────────────────┤
│                fresnel-fir-ir                        │  Core types, expression AST,
│                                                     │  JSON parsing
└─────────────────────────────────────────────────────┘
```

| Crate | Purpose |
|-------|---------|
| `fresnel-fir-ir` | Core IR types and expression AST. Parses JSON specifications with structured predicates (no string-based expressions). |
| `fresnel-fir-compiler` | Validates IR, compiles expressions into type-checked predicates, builds NDA protocol graphs, enforces nesting depth limits. |
| `fresnel-fir-model` | Copy-on-write model state, effect application, refinement predicate evaluation, invariant checking, temporal property verification, simulation. |
| `fresnel-fir-sandbox` | Wasmtime-based WASM runtime. Fuel metering, memory limits, snapshot/restore, function interception, crash containment. |
| `fresnel-fir-vif` | Verification infrastructure. Bridges model and sandbox: executes DUT actions, queries observers, validates interfaces, tags results. |
| `fresnel-fir-explore` | Traversal engine (NDA graph walking), constraint solver (fracture/solve/abort), test vector pool, adaptive exploration (decay, coverage floors, unreachability proofs). |
| `fresnel-fir-core` | Campaign orchestration, finding accumulation, analytics (coverage velocity, finding rates), resource limits, cross-campaign memory, MCP server. |

## Getting Started

### Prerequisites

- **Rust stable toolchain** (1.70+) — install via [rustup](https://rustup.rs/)
- Components: `rustfmt`, `clippy` (installed by default with rustup)

### Clone and verify

```bash
git clone https://github.com/ajhcs/Beacon.git
cd Beacon
cargo test --workspace
```

All 281 tests should pass. Build time from scratch is approximately 90 seconds.

### Run targeted test suites

```bash
# MCP server protocol tests
cargo test -p fresnel-fir-core --test mcp_tests

# End-to-end traversal campaigns
cargo test -p fresnel-fir-explore --test traversal_tests

# DUT integration lifecycle
cargo test -p fresnel-fir-vif --test integration_tests
```

### Build for release

```bash
cargo build --workspace --release
```

## How It Works

### The Two-Loop Architecture

FresnelFir operates in two loops:

**Outer loop (specification refinement):** An AI agent asks Socratic questions to extract your intent, generates a formal IR specification, and iterates with you until the spec captures what you actually want. This loop involves a human.

**Inner loop (verification):** FresnelFir compiles the IR, loads your code into the WASM sandbox, and autonomously explores the state space — generating test vectors, traversing protocol paths, checking invariants, and reporting findings. The AI agent can then fix violations and re-verify without human intervention.

### The IR Specification

Specifications are JSON documents with nine required sections:

```json
{
  "entities": { },
  "refinements": { },
  "functions": { },
  "protocols": { },
  "effects": { },
  "properties": { },
  "generators": { },
  "exploration": { },
  "inputs": { },
  "bindings": { }
}
```

- **entities** define the data types in your domain (User, Document, Session)
- **refinements** constrain field values (role must be "admin" or "viewer")
- **functions** define pure model computations and DUT observers
- **protocols** define valid operation sequences using grammar constructs
- **effects** specify what each action does to the model state
- **properties** declare invariants and temporal rules
- **generators** configure how test inputs are produced
- **exploration** sets traversal strategy and coverage targets
- **inputs/bindings** wire the spec to your actual WASM module

See [`docs/beacon-ir-schema.md`](docs/beacon-ir-schema.md) for the complete schema reference with examples.

### The Solver: Fracture/Solve/Abort

FresnelFir doesn't just fuzz randomly. Its constraint solver uses a three-phase strategy from the original patents:

1. **Fracture** — Partition the input domain into subspaces along each variable's possible values
2. **Solve** — Within each subspace, use SAT solving to find concrete inputs satisfying all constraints
3. **Abort** — If a subspace is unsatisfiable, prove it and skip it permanently

This gives you the thoroughness of exhaustive testing with the speed of directed search.

## Repository Layout

```
Cargo.toml                    Workspace definition
crates/                       All seven Rust crates
docs/
  beacon-ir-schema.md         Complete IR specification reference
  architecture.md             System architecture overview
  troubleshooting.md          Common issues and solutions
  plans/                      Design documents and implementation plans
.github/
  workflows/ci.yml            CI pipeline (fmt, clippy, test, security)
  workflows/release.yml       Release packaging and publishing
scripts/
  release-smoke.ps1           Local pre-release validation
CHANGELOG.md                  Release history
CONTRIBUTING.md               Contributor guidelines
SECURITY.md                   Vulnerability reporting
LICENSE                       MIT license
```

## Quality and CI

Every push to `master` runs:
- `cargo fmt --all -- --check` — formatting
- `cargo clippy --workspace --all-targets -- -D warnings` — lints as errors
- `cargo test --workspace --locked` — full test suite (281 tests)
- `cargo audit` — dependency vulnerability scan
- `cargo deny check advisories` — dependency policy enforcement
- `gitleaks detect --redact` — secret scanning

## Current Status

**Version:** 0.1.0 (initial GA release)

FresnelFir is pre-1.0. APIs may change between minor versions. The core verification pipeline is complete and tested:

- IR parsing and validation
- Predicate compilation and protocol graph generation
- Model state management with copy-on-write snapshots
- WASM sandbox with fuel metering and snapshot/restore
- Constraint-based test vector generation (fracture/solve/abort)
- NDA graph traversal with adaptive exploration
- Temporal property checking and invariant enforcement
- Campaign management with analytics and resource limits
- MCP server for AI agent integration
- Cross-campaign learning and regression tracking

**Test coverage:** 281 tests across all seven crates, zero failures.

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Language | Rust 2021 edition |
| WASM runtime | Wasmtime v41 |
| SAT solver | Varisat (pure Rust) |
| Parallelism | Rayon (data parallelism), Crossbeam (lock-free structures) |
| Async runtime | Tokio (MCP server only) |
| Serialization | serde + serde_json |
| RNG | rand_chacha (deterministic, seedable) |

## Heritage

FresnelFir's name honors its origins:

- **Fresnel** — from [Lighthouse Design Automation](https://en.wikipedia.org/wiki/Fresnel_lens), where the core verification patents were developed. A Fresnel lens focuses light into a directed beam; FresnelFir focuses verification onto code.
- **Fir** — from Silicon Forest Research, the original patent holder. Silicon Forest is the technology hub of the Pacific Northwest, named for the Douglas fir trees of Oregon.

The four US patents covering NDA traversal, fracture/solve/abort, and modification directives form the algorithmic foundation of FresnelFir's exploration engine.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development workflow, quality gates, and how to submit changes.

## Security

See [SECURITY.md](SECURITY.md) for vulnerability reporting procedures.

## License

[MIT](LICENSE)
