# Release Checklist (`v0.1.0`)
Status Legend: `NOT STARTED` | `IN PROGRESS` | `PASS` | `FAIL`

## G0: Security and Supply Chain
Owner: Engineering
Evidence: `docs/release/security-baseline.md`, `docs/release/rc1-evidence.md`, `docs/release/rc2-evidence.md`
Status: PASS

- [x] Dependency audit passes in CI.
- [x] Secret scan passes on tree and scoped history in CI.
- [x] Dependency policy/advisory check passes in CI (`cargo-deny`).
- [x] Locked dependency mode is enforced in CI/release workflow definitions.
- [x] CI run URLs and SHAs are recorded in RC evidence docs.

## G1: Code Quality
Owner: Engineering
Evidence: `docs/release/baseline-audit-2026-02-17.md`, `docs/release/rc1-evidence.md`, `docs/release/rc2-evidence.md`
Status: PASS

- [x] `cargo fmt --all -- --check` passes.
- [x] `cargo clippy --workspace --all-targets --locked -- -D warnings` passes.
- [x] `cargo test --workspace --locked` passes.
- [x] No unresolved release-critical TODOs.

## G2: Documentation
Owner: Docs
Evidence: `docs/release/rc1-evidence.md`, `docs/release/rc2-evidence.md`
Status: PASS

- [x] Required top-level docs exist and are internally consistent.
- [x] Docs checks pass (broken links, command snippet validation).
- [x] Fresh-clone quickstart validated by non-author. `git clone` + `cargo test --workspace --locked` = 281/281 pass on MINGW64/Windows (2026-02-17).

## G3: Automation and CI/CD
Owner: Engineering
Evidence: `docs/release/rc1-evidence.md`, `docs/release/rc2-evidence.md`
Status: PASS

- [x] Required CI checks are green on protected default branch.
- [x] Tier 1 matrix (`windows-latest`, `ubuntu-latest`) is green.
- [x] Release workflow validates on RC tag.
- [x] Installer artifact path validation steps are present in release workflow definition.
- [x] Installer path validation has passing Tier 1 run evidence attached.

## G4: User Validation
Owner: Engineering + User (`ajhcs`)
Evidence: `docs/release/final-triage.md`, `docs/release/rc2-evidence.md`
Status: PASS

- [x] Final user test cohort completed. G4 sweep: `release-smoke.ps1` 281/281, `mcp_tests` 15/15, `traversal_tests` 12/12, `integration_tests` 4/4 (2026-02-17).
- [x] No unresolved `critical`/`high` user findings. F-003 CLOSED.
- [x] Deferred issues documented with mitigation and owner.

## G5: Publish Readiness
Owner: TBD
Evidence: `docs/release/rollback-drill.md`, `docs/release/post-release-smoke.md`
Status: NOT STARTED

- [ ] Changelog and release notes finalized.
- [ ] Rollback/hotfix drill completed.
- [ ] Final sign-off captured from release owner and engineering reviewer.
