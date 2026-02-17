# Beacon

Beacon is a Rust workspace for declarative verification of behavior-driven systems, with a strong focus on model-guided exploration and WASM-hosted DUT (device under test) integration.

The repository is organized as composable libraries:
- `beacon-ir`: IR types and parsing.
- `beacon-compiler`: IR validation and graph/predicate compilation.
- `beacon-model`: model state, effects, invariant and temporal checks, simulation.
- `beacon-sandbox`: constrained Wasmtime sandboxing and snapshot/restore.
- `beacon-vif`: verification interface adapter and DUT interface validation.
- `beacon-explore`: traversal engine, solver pipeline, adaptive directives.
- `beacon-core`: campaign management, analytics, MCP-style tool request handling.

## Maturity

Beacon is currently `0.1.0` and pre-1.0.
- APIs are expected to change.
- The project is library-first; no stable CLI binary is published yet.
- Existing tests are extensive across crates, but lint/format cleanup is still in progress.

## Quickstart

### Prerequisites

- Rust stable toolchain (`rustup`, `cargo`)
- Rust components: `rustfmt`, `clippy`

### Get and verify

```bash
git clone <repo-url>
cd Beacon
cargo test --workspace
```

### Run focused suites

```bash
cargo test -p beacon-core --test mcp_tests
cargo test -p beacon-explore --test traversal_tests
cargo test -p beacon-vif --test integration_tests
```

## Test and quality commands

```bash
# Full workspace tests
cargo test --workspace

# Formatting check
cargo fmt --all -- --check

# Lints as errors
cargo clippy --workspace --all-targets -- -D warnings
```

## Repository layout

- `Cargo.toml`: workspace definition and shared dependency versions.
- `crates/`: all Beacon libraries.
- `docs/beacon-ir-schema.md`: IR schema reference.
- `docs/plans/`: internal planning/design notes.
- `README.md`: project overview.
- `CONTRIBUTING.md`: contributor workflow and quality gates.
- `SECURITY.md`: vulnerability reporting process.
- `CODE_OF_CONDUCT.md`: community behavior policy.
- `CHANGELOG.md`: release history and planned release notes.

## Additional docs

- `docs/architecture.md`
- `docs/troubleshooting.md`
