# Contributing to FresnelFir

Thanks for contributing. This repository is a Rust workspace; contributions should be small, focused, and test-backed.

## Development setup

1. Install Rust stable (`rustup`).
2. Ensure formatting and lint tools are installed:
```bash
rustup component add rustfmt clippy
```
3. From repo root, verify tests:
```bash
cargo test --workspace
```

## Quality gates

Run these before opening a PR:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

If a failure is caused by unrelated pre-existing issues, do not hide it. Note it clearly in the PR description with file references.

## Pull request expectations

- Keep PR scope tight and coherent.
- Add or update tests for behavior changes.
- Update docs when user-facing behavior or workflows change.
- Avoid unrelated refactors/format churn.
- Include a short verification section in the PR body listing commands run and pass/fail status.
- Add a `CHANGELOG.md` update when the change should be visible in release notes.

## Commit and review notes

- Prefer clear, imperative commit messages.
- If the change touches multiple crates, explain cross-crate impact in the PR.
- If adding a new dependency, explain why existing dependencies are insufficient.
