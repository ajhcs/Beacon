# Plan: Initial GitHub Release Readiness

**Generated**: 2026-02-17
**Estimated Complexity**: High

## Overview
Prepare Beacon for its first public GitHub release by running a structured readiness pass across code quality, bug fixing, automated testing, documentation, and final user validation. The plan is designed for small, committable increments and finishes with a tagged release and rollback-ready release process.

Current baseline from repository examination:
- `cargo test --workspace` passes.
- Release-critical assets are missing (`README.md`, `.github/` workflows).
- Existing compiler warnings should be resolved before first release.

## Prerequisites
- Rust toolchain installed (`stable`) and reproducible in CI.
- GitHub repository with permission to create tags/releases.
- Team agreement on first release scope (library-only vs runnable product walkthrough).
- A small user-test cohort (internal or external) for final validation.

## Sprint 1: Release Readiness Audit
**Goal**: Define release scope and produce a factual baseline report.
**Demo/Validation**:
- Run baseline checks and publish a readiness report.
- Review and approve release acceptance criteria.

### Task 1.1: Define v0.1.0 Scope and Acceptance Criteria
- **Location**: `docs/release/release-criteria.md`
- **Description**: Document what "ready for initial GitHub release" means (must-pass checks, must-have docs, known limitations, and out-of-scope items).
- **Dependencies**: None
- **Acceptance Criteria**:
  - v0.1.0 scope, non-goals, and release gate checklist are documented.
  - Criteria include code quality, docs quality, and user-testing exit conditions.
- **Validation**:
  - Team sign-off on `docs/release/release-criteria.md`.

### Task 1.2: Create Baseline Health Report
- **Location**: `docs/release/baseline-health.md`
- **Description**: Capture outputs from `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.
- **Dependencies**: Task 1.1
- **Acceptance Criteria**:
  - Report includes pass/fail status and concrete gaps blocking release.
  - Report references any warnings, flaky tests, or unstable behaviors.
- **Validation**:
  - Re-run commands and verify report remains accurate.

### Task 1.3: Release Asset Gap Analysis
- **Location**: `docs/release/release-assets.md`
- **Description**: List required assets for first release (README, CONTRIBUTING, CHANGELOG, LICENSE verification, workflows, issue templates, release notes template).
- **Dependencies**: Task 1.1
- **Acceptance Criteria**:
  - Missing assets are identified with owners and priorities.
  - Asset list maps directly to Sprint 3 and Sprint 4 tasks.
- **Validation**:
  - File-by-file checklist marked complete/incomplete.

## Sprint 2: Bug Fixing and Code Hardening
**Goal**: Remove known quality risks and lock in regressions with tests.
**Demo/Validation**:
- All warnings resolved or intentionally documented with rationale.
- Regression tests added for every bug fix.

### Task 2.1: Fix Current Compiler Warnings
- **Location**: `crates/beacon-vif/src/adapter.rs`, `crates/beacon-explore/src/solver/domain.rs`
- **Description**: Remove unused field/import warnings; keep code warning-free under strict clippy/build settings.
- **Dependencies**: Task 1.2
- **Acceptance Criteria**:
  - `cargo clippy --workspace --all-targets -- -D warnings` passes.
- **Validation**:
  - CI-equivalent local command passes with zero warnings.

### Task 2.2: Resolve Open Traversal TODO
- **Location**: `crates/beacon-explore/src/traversal/engine.rs`
- **Description**: Replace placeholder model-state hash logic with intended computation path or create a tracked issue with explicit constraints if deferring.
- **Dependencies**: Task 1.2
- **Acceptance Criteria**:
  - TODO is removed, or deferral is documented in release known issues with rationale.
  - Behavior is covered by deterministic traversal tests.
- **Validation**:
  - Targeted tests in `crates/beacon-explore/tests/traversal_tests.rs` pass.

### Task 2.3: Add Regression Tests for Fixes
- **Location**: `crates/beacon-vif/tests/`, `crates/beacon-explore/tests/`
- **Description**: Add tests for each bug fix and edge case identified during hardening.
- **Dependencies**: Task 2.1, Task 2.2
- **Acceptance Criteria**:
  - Each bug fix has at least one failing-before/passing-after test case.
- **Validation**:
  - `cargo test --workspace` passes and includes new test coverage.

### Task 2.4: Panic/Failure-Path Audit
- **Location**: `crates/**/src/*.rs`, `docs/release/known-risks.md`
- **Description**: Confirm no user-facing runtime paths rely on unchecked panic patterns; document intentional panics in tests only.
- **Dependencies**: Task 1.2
- **Acceptance Criteria**:
  - Any non-test panic usage is eliminated or justified.
  - Known risks list is created for deferred hardening items.
- **Validation**:
  - Search-based audit plus focused negative-path tests pass.

## Sprint 3: Automated Quality Gates and GitHub Workflows
**Goal**: Ensure every PR and release candidate is automatically validated.
**Demo/Validation**:
- CI runs on pull requests and main branch pushes.
- Release workflow supports tag-based publication flow.

### Task 3.1: Add CI Workflow
- **Location**: `.github/workflows/ci.yml`
- **Description**: Add workflow for formatting, clippy, tests, and workspace build.
- **Dependencies**: Sprint 2 complete
- **Acceptance Criteria**:
  - CI runs on PR and push.
  - Fails on formatting/lint/test issues.
- **Validation**:
  - Open test PR and verify CI gate behavior.

### Task 3.2: Add Release Workflow
- **Location**: `.github/workflows/release.yml`
- **Description**: Create a tag-triggered workflow that builds release artifacts and publishes GitHub release notes.
- **Dependencies**: Task 3.1
- **Acceptance Criteria**:
  - Tag pattern (for example `v*`) triggers release workflow.
  - Workflow attaches build artifacts and generated notes draft.
- **Validation**:
  - Dry run on pre-release tag (for example `v0.1.0-rc1`).

### Task 3.3: Add Repository Hygiene Templates
- **Location**: `.github/ISSUE_TEMPLATE/`, `.github/pull_request_template.md`
- **Description**: Add issue and PR templates with reproduction, verification, and risk fields.
- **Dependencies**: Task 3.1
- **Acceptance Criteria**:
  - New issues/PRs use templates with test evidence sections.
- **Validation**:
  - Create sample issue and PR to confirm template rendering.

## Sprint 4: Documentation and Onboarding for First-Time Users
**Goal**: Make the repository usable by external users without private context.
**Demo/Validation**:
- A new user can clone, build, test, and understand project purpose quickly.
- Release notes and changelog are ready.

### Task 4.1: Create Public README
- **Location**: `README.md`
- **Description**: Add project summary, architecture snapshot, quickstart, test commands, and current maturity statement.
- **Dependencies**: Task 1.3
- **Acceptance Criteria**:
  - README includes "what it is", "who it is for", "how to run", and "limitations".
- **Validation**:
  - Fresh-reader walkthrough confirms README is sufficient to run `cargo test --workspace`.

### Task 4.2: Add Contribution and Development Guide
- **Location**: `CONTRIBUTING.md`, `docs/development.md`
- **Description**: Define coding standards, test expectations, commit/PR requirements, and local dev setup.
- **Dependencies**: Task 3.1, Task 4.1
- **Acceptance Criteria**:
  - Contributors can follow docs to submit a passing PR.
- **Validation**:
  - Trial run using docs-only instructions on clean clone.

### Task 4.3: Add Changelog and Release Notes Template
- **Location**: `CHANGELOG.md`, `docs/release/release-notes-template.md`
- **Description**: Establish initial changelog format and draft v0.1.0 notes.
- **Dependencies**: Sprint 2 complete
- **Acceptance Criteria**:
  - v0.1.0 entries include features, fixes, known limitations, and upgrade notes.
- **Validation**:
  - Draft release notes reviewed against merged changes.

## Sprint 5: End-to-End Verification and Final User Testing
**Goal**: Validate real-user readiness and close remaining release blockers.
**Demo/Validation**:
- Release candidate passes full verification suite.
- User testing findings are triaged and resolved or accepted with rationale.

### Task 5.1: Build Release Candidate Checklist Run
- **Location**: `docs/release/rc-checklist.md`, `scripts/release-smoke.ps1`
- **Description**: Execute all release checks in one reproducible sequence and record results.
- **Dependencies**: Sprint 3, Sprint 4 complete
- **Acceptance Criteria**:
  - Checklist run is reproducible by another team member.
  - All critical checks pass for `v0.1.0-rc1`.
- **Validation**:
  - Independent rerun from clean environment.

### Task 5.2: Final User Testing Session
- **Location**: `docs/release/user-test-plan.md`, `docs/release/user-test-results.md`
- **Description**: Run scripted user tasks (install/build/test/inspect findings pipeline) with target users and capture friction points.
- **Dependencies**: Task 5.1
- **Acceptance Criteria**:
  - User test report includes observed failures, severity, and resolution decision.
  - No unresolved critical issues remain.
- **Validation**:
  - Retest all issues marked fixed.

### Task 5.3: Final Bug Triage and Stabilization
- **Location**: `docs/release/final-triage.md`, affected crate source/tests
- **Description**: Fix critical/high findings from user testing, defer non-critical issues with explicit tracking.
- **Dependencies**: Task 5.2
- **Acceptance Criteria**:
  - Critical/high issues closed.
  - Deferred items captured in known issues with mitigation notes.
- **Validation**:
  - Full CI pass and targeted regression pass for each fix.

## Sprint 6: Initial GitHub Release
**Goal**: Publish v0.1.0 with traceable artifacts and rollback path.
**Demo/Validation**:
- Git tag and GitHub release published.
- Post-release verification confirms release integrity.

### Task 6.1: Version and Tag Preparation
- **Location**: `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`
- **Description**: Confirm version metadata, freeze changelog, and prepare release tag.
- **Dependencies**: Sprint 5 complete
- **Acceptance Criteria**:
  - Version and changelog are consistent with release notes.
  - Release commit is signed off.
- **Validation**:
  - Tag dry-run checklist completed.

### Task 6.2: Publish GitHub Release
- **Location**: GitHub Releases, `.github/workflows/release.yml`
- **Description**: Push tag, run release workflow, attach artifacts, and publish final notes.
- **Dependencies**: Task 6.1
- **Acceptance Criteria**:
  - Release workflow succeeds.
  - Downloadable artifacts and notes are publicly available.
- **Validation**:
  - Verify release page and artifact checksums.

### Task 6.3: Post-Release Smoke Verification
- **Location**: `docs/release/post-release-check.md`
- **Description**: Validate clone/build/test experience from published tag and monitor first incoming issues.
- **Dependencies**: Task 6.2
- **Acceptance Criteria**:
  - Post-release smoke run passes on at least one clean environment.
  - Any immediate hotfix path is documented.
- **Validation**:
  - Recorded smoke log and issue triage notes.

## Testing Strategy
- Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` on every PR.
- Add targeted regression tests for each bug fix before merge.
- Maintain one release-candidate checklist that runs all gating checks in order.
- Include user-journey validation: fresh clone, build, test, and minimal usage flow from docs only.
- Require green CI plus successful user test retest before tagging release.

## Potential Risks & Gotchas
- Ambiguous release target (framework code vs end-user runnable product) can stall doc and user test quality.
- Missing GitHub automation may allow regressions between final fixes and release tagging.
- User-testing cohort that is too familiar with the project can hide onboarding friction.
- Late bug-fix merges can invalidate release notes and checklist evidence.
- Deferring TODO/risk items without explicit known-issues tracking can create trust problems on initial release.

## Rollback Plan
- Keep release tags immutable and create hotfix branch from release commit if severe issues are found.
- If release workflow fails after tagging, mark release as draft/unpublished, patch on hotfix branch, and retag with corrected version (for example `v0.1.1`).
- For critical regressions, publish rollback advisory in release notes and pin users to last known good tag.
- Preserve `docs/release/*` evidence for each release candidate to support fast incident triage.
