# FresnelFir architecture

## Purpose

FresnelFir is a multi-crate Rust verification harness centered on a declarative IR. The system compiles IR into executable structures, explores protocol paths, executes bound DUT actions, and records coverage/findings.

## High-level flow

1. Parse IR JSON into typed structures (`fresnel-fir-ir`).
2. Validate and compile protocols/predicates (`fresnel-fir-compiler`).
3. Prepare model state and property checks (`fresnel-fir-model`).
4. Optionally load and isolate WASM DUT (`fresnel-fir-sandbox`).
5. Map abstract actions to concrete exports (`fresnel-fir-vif`).
6. Run traversal/solver/adaptation loops (`fresnel-fir-explore`).
7. Track campaigns and expose MCP-style tool calls (`fresnel-fir-core`).

## Crate responsibilities

- `crates/beacon-ir`
- IR data types (`types`), expression format (`expr`), and parsing (`parse`).

- `crates/beacon-compiler`
- Structural validation (`validate`), graph compilation (`graph`, `protocol`), predicate compilation (`predicate`).

- `crates/beacon-model`
- State model (`state`), effect application (`effect`), expression evaluation (`eval`), invariant and temporal checks (`invariant`, `temporal`), simulation (`simulate`).

- `crates/beacon-sandbox`
- Wasmtime-based module loading/execution, resource limits, and snapshot/restore support.

- `crates/beacon-vif`
- Action/observer adapter from IR bindings to sandbox calls, interface and signature validation.

- `crates/beacon-explore`
- Traversal engine, strategy stack, coverage signals, solver pipeline, and adaptive directives.

- `crates/beacon-core`
- Campaign lifecycle, budgets, findings/coverage/analytics storage, and JSON-RPC MCP tool request handling.

## Interfaces and boundaries

- `fresnel-fir-core` does not currently expose a standalone server binary in this repository.
- Integration is library-driven through crate APIs and tests.
- MCP behavior is implemented as request/response handlers in `crates/beacon-core/src/mcp.rs`.

## Current maturity notes

- Version is pre-1.0 (`0.1.0` workspace version).
- Test coverage is broad across crates.
- Lint/format cleanup is ongoing in parts of the codebase.
