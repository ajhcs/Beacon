# Troubleshooting

## `cargo fmt --all -- --check` fails

If formatting fails, run:

```bash
cargo fmt --all
```

Then rerun the check command.

## `cargo clippy --workspace --all-targets -- -D warnings` fails

This command treats all warnings as errors. Fix the reported lint in your change set first.

If the failure is from unrelated pre-existing code, document it in your PR with:
- exact error text
- file path
- confirmation that your change does not increase scope of the issue

## `cargo test --workspace` is slow or appears to hang

Common causes:
- first build of large dependencies (for example `wasmtime`)
- lock contention in shared Cargo cache/build directories

Actions:
- wait for dependency compilation to complete
- rerun once to benefit from incremental build cache

## IR compile/validation errors

The compiler validates cross-section consistency. Frequent causes:
- protocol action exists without matching `effects` entry
- protocol action exists without matching `bindings.actions` entry
- protocol references a missing protocol name
- `alt` branches all have zero weight
- `repeat` has `min > max`

Use fixture-driven tests to compare valid shape:
- `crates/beacon-ir/tests/fixtures/document_lifecycle.json`

## WASM binding or execution failures

Typical failures:
- missing export name in module (`beacon-vif` interface validation error)
- export exists but is not a function
- parameter/return signature mismatch
- fuel exhaustion during execution (timeout-like behavior)

Check:
- binding names in IR `bindings.actions`
- exported function list from the DUT module
- sandbox limits in `crates/beacon-sandbox/src/config.rs`
