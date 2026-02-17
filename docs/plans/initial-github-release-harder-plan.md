# Plan: Initial GitHub Release Hardening

**Generated**: 2026-02-17
**Estimated Complexity**: High

## Overview
Prepare Beacon for its first public GitHub release (`v0.1.0`) using a gated release program that covers code examination, bug fixing, automated validation, documentation, CI/CD, and final user testing. The plan enforces measurable release gates and a strict blocker policy.

Observed baseline (from current repo state):
- `cargo test --workspace` passes.
- `cargo fmt --all -- --check` fails (format drift across many files).
- `cargo clippy --workspace --all-targets -- -D warnings` fails with real lint blockers in `beacon-sandbox`.
- Release assets missing: `README.md`, `LICENSE`, `CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `.github/workflows/*`.
- Known TODO in traversal path selection (`crates/beacon-explore/src/traversal/engine.rs`).

## Prerequisites
- GitHub repo admin access (branch protection, workflow permissions, release creation).
- Rust stable toolchain pinned in CI.
- Named release owner and two reviewers (engineering + docs).
- Release deliverable scope fixed for `v0.1.0`:
  - source + pre-built artifacts + installer
- Platform support policy fixed for `v0.1.0`:
  - Tier 1 (release-blocking): `windows-latest`, `ubuntu-latest`
  - Tier 2 (best-effort): `macos-latest` (non-blocking in `v0.1.0`)
- TODO policy fixed for `v0.1.0`:
  - No unresolved TODOs allowed in release-critical runtime paths.
  - TODOs allowed only in tests/docs if linked to tracked issue and non-blocking.
- Test cohort availability for final user testing (minimum 3 participants).
- Security scanning tools available in CI and local release runbook:
  - dependency audit tool
  - secret scanning tool

## Release Gates (Definition of Done)
- `G0` Security and supply-chain gate:
  - Dependency audit passes.
  - Secret scan passes on current tree and relevant history.
  - License/dependency policy check passes.
  - Locked dependency mode is enforced in release checks.
- `G1` Code quality gate:
  - `cargo fmt --all -- --check` passes.
  - `cargo clippy --workspace --all-targets --locked -- -D warnings` passes.
  - `cargo test --workspace --locked` passes.
- `G2` Documentation gate:
  - Required top-level docs exist and are internally consistent.
  - Docs checks pass:
    - broken link check
    - command snippet validation for quickstart/test commands
  - Fresh-clone quickstart validated by non-author.
- `G3` Automation gate:
  - CI required checks configured and green on protected default branch.
  - Tier 1 platform matrix checks pass (`windows-latest`, `ubuntu-latest`).
  - Release workflow validates tag-driven release path.
  - Installer package/build path validates successfully.
- `G4` User validation gate:
  - Final user test run complete with no unresolved critical/high findings.
- `G5` Publish gate:
  - Changelog + release notes finalized.
  - Rollback/hotfix path validated.

## Critical Path
1. Remove clippy blockers and formatting drift.
2. Add/verify CI gates.
3. Build release docs and onboarding docs.
4. Run release-candidate checks and user tests.
5. Triage/fix high-severity findings.
6. Tag and publish `v0.1.0`.

## Sprint 1: Program Setup and Baseline Freeze
**Goal**: Convert current state into a managed release program with explicit blockers and acceptance criteria.
**Demo/Validation**:
- Release board created with blocker severity and owners.
- Baseline report captured and approved.

### Task 1.1: Define Release Scope Contract
- **Location**: `docs/release/release-scope-v0.1.0.md`
- **Description**: Define in-scope vs out-of-scope capabilities for `v0.1.0`, including known limitations and support expectations.
- **Complexity**: 3
- **Dependencies**: None
- **Acceptance Criteria**:
  - Scope document has release objective, non-goals, and support statement.
  - Document is signed off by release owner.
- **Validation**:
  - PR approval by engineering lead and docs reviewer.

### Task 1.2: Baseline Technical Audit Snapshot
- **Location**: `docs/release/baseline-audit-2026-02-17.md`
- **Description**: Record exact results for fmt, clippy, tests, TODO scan, panic scan, and missing release assets.
- **Complexity**: 2
- **Dependencies**: Task 1.1
- **Acceptance Criteria**:
  - Audit includes command lines, pass/fail status, and blocker list.
- **Validation**:
  - Re-run sampled commands and match recorded outcomes.

### Task 1.3: Blocker Tracker and Severity Policy
- **Location**: `docs/release/blockers.md`
- **Description**: Define severity taxonomy (`critical`, `high`, `medium`, `low`) and release blocking policy.
- **Complexity**: 2
- **Dependencies**: Task 1.2
- **Acceptance Criteria**:
  - Blocking rules for release candidate and GA are explicit.
  - Every baseline issue is mapped to a severity.
- **Validation**:
  - Reviewer can classify a sample issue using policy without ambiguity.

### Task 1.4: Create Release Checklist Skeleton
- **Location**: `docs/release/release-checklist.md`
- **Description**: Build checklist sections aligned to gates `G0`-`G5` with owner and evidence fields.
- **Complexity**: 3
- **Dependencies**: Task 1.3
- **Acceptance Criteria**:
  - Checklist references all required commands and documents.
  - Each gate has clear pass/fail criteria.
- **Validation**:
  - Dry-run checklist completion without code changes.

### Task 1.5: Lock Release Decisions (Scope, Platforms, TODO Policy)
- **Location**: `docs/release/release-decisions-v0.1.0.md`
- **Description**: Record and freeze three release decisions:
  - artifact scope: `source+prebuilt+installer`
  - platform matrix: Tier 1 `windows-latest` + `ubuntu-latest`; Tier 2 `macos-latest` best-effort
  - TODO policy: zero unresolved release-critical TODOs; limited test/docs TODO exceptions with issue links
- **Complexity**: 3
- **Dependencies**: Task 1.1, Task 1.3
- **Acceptance Criteria**:
  - Decisions are explicit, approved, and referenced by downstream tasks.
- **Validation**:
  - Reviewer confirms no downstream task has unresolved policy ambiguity.

### Task 1.6: Security and Secret-Scan Baseline
- **Location**: `docs/release/security-baseline.md`
- **Description**: Establish baseline results for dependency audit, license/dependency policy, and secret scanning.
- **Complexity**: 4
- **Dependencies**: Task 1.2
- **Acceptance Criteria**:
  - Security baseline captures pass/fail status and remediation owners.
- **Validation**:
  - Repeat scan confirms reproducible baseline outcomes.

## Sprint 2: Code Hardening and Bug-Fix Pass
**Goal**: Reach warning-free quality gate and close known technical debt that can impact first release trust.
**Demo/Validation**:
- Clippy and fmt pass with strict settings.
- New regression tests cover hardening fixes.

### Task 2.1: Apply Workspace Formatting in Isolated Change
- **Location**: `crates/**`, `docs/**`
- **Description**: Run formatter and land formatting-only commit before semantic fixes.
- **Complexity**: 4
- **Dependencies**: Task 1.2
- **Acceptance Criteria**:
  - `cargo fmt --all -- --check` passes.
  - Commit contains formatting-only changes.
- **Validation**:
  - `cargo test --workspace` after formatting commit.

### Task 2.2: Fix Clippy Blockers in Sandbox
- **Location**: `crates/beacon-sandbox/src/sandbox.rs`, `crates/beacon-sandbox/src/snapshot.rs`
- **Description**: Resolve current lint blockers (`clone_on_copy`, `manual_div_ceil`) with behavior-preserving changes.
- **Complexity**: 3
- **Dependencies**: Task 2.1
- **Acceptance Criteria**:
  - Both current errors removed.
  - No behavior regression in sandbox tests.
- **Validation**:
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test -p beacon-sandbox --locked`

### Task 2.3: Resolve Traversal TODO or Formally Defer
- **Location**: `crates/beacon-explore/src/traversal/engine.rs`, `docs/release/known-issues.md`
- **Description**: Implement model-state hash path for branch weight conditioning, or defer with rationale and risk impact.
- **Complexity**: 6
- **Dependencies**: Task 1.2, Task 1.5
- **Acceptance Criteria**:
  - If in release-critical path, TODO is removed before GA.
  - Deferral allowed only when policy-compliant (non-critical test/docs paths with issue link and mitigation).
- **Validation**:
  - Targeted traversal tests pass and behavior is deterministic.

### Task 2.4: Panic and Failure-Path Audit
- **Location**: `crates/**/src/*.rs`, `crates/**/bin/*.rs`, `examples/**`, `scripts/**`, `docs/release/failure-path-audit.md`
- **Description**: Ensure runtime paths avoid uncontrolled panic usage; panic is allowed in tests only.
- **Complexity**: 5
- **Dependencies**: Task 2.2
- **Acceptance Criteria**:
  - Non-test panic usage audited and either removed or documented as unreachable.
- **Validation**:
  - Search audit + targeted negative-path tests.

### Task 2.5: Regression Test Additions for Every Fix
- **Location**: `crates/beacon-explore/tests/`, `crates/beacon-sandbox/tests/`
- **Description**: Add tests that would have caught issues addressed in Sprint 2.
- **Complexity**: 5
- **Dependencies**: Task 2.2, Task 2.3, Task 2.4
- **Acceptance Criteria**:
  - Each fixed issue has at least one explicit regression test.
- **Validation**:
  - `cargo test --workspace --locked`

## Sprint 3: CI/CD and Repository Governance
**Goal**: Enforce release quality automatically on every PR and release event.
**Demo/Validation**:
- CI is required and green.
- Release workflow can produce release artifacts/notes from tag.

### Task 3.1: Implement CI Workflow with Required Checks
- **Location**: `.github/workflows/ci.yml`
- **Description**: Add PR/push workflow for fmt, clippy, tests, with required Tier 1 matrix (`windows-latest`, `ubuntu-latest`) and optional Tier 2 (`macos-latest`) informational job.
- **Complexity**: 5
- **Dependencies**: Sprint 2 complete, Task 1.5
- **Acceptance Criteria**:
  - CI fails on any gate regression.
  - Checks are named and stable for branch protection.
  - Tier 1 checks are required in branch protection settings.
- **Validation**:
  - Open test PR and verify required checks behavior.

### Task 3.2: Add Tag-Driven Release Workflow
- **Location**: `.github/workflows/release.yml`
- **Description**: Add release pipeline for `v*` tags including build, checksum generation, pre-built artifact upload, installer generation, release notes draft.
- **Complexity**: 6
- **Dependencies**: Task 3.1, Task 1.5
- **Acceptance Criteria**:
  - Dry-run tag produces expected artifacts and notes.
  - Tier 1 pre-built artifacts are published.
  - Artifacts are installable and runnable from release output.
  - Installer executes successfully and installs runnable release.
- **Validation**:
  - Execute on `v0.1.0-rc1` or equivalent pre-release tag.
  - Download artifact and run smoke command from artifact packaging.
  - Validate installer + smoke on `windows-latest` and `ubuntu-latest`.
  - Run installer in clean environment and execute smoke command post-install.

### Task 3.3: Add Repo Metadata and Templates
- **Location**: `.github/ISSUE_TEMPLATE/*.yml`, `.github/pull_request_template.md`, `.github/CODEOWNERS`
- **Description**: Add issue/PR templates with reproducibility fields; define ownership for release-critical paths.
- **Complexity**: 3
- **Dependencies**: Task 3.1
- **Acceptance Criteria**:
  - New issues/PRs enforce test evidence and risk declaration.
- **Validation**:
  - Manual creation of sample issue/PR confirms template rendering.

### Task 3.4: Configure Branch Protection Settings
- **Location**: `docs/release/branch-protection.md` (recorded policy)
- **Description**: Require PR reviews and green CI checks before merge to release branch.
- **Complexity**: 2
- **Dependencies**: Task 3.1
- **Acceptance Criteria**:
  - Branch protection policy documented and applied.
- **Validation**:
  - Attempt merge with failing checks is blocked.

## Sprint 4: Public Documentation and Onboarding
**Goal**: Make repository understandable and runnable without private project context.
**Demo/Validation**:
- Independent reader can clone, run tests, and understand architecture and limitations.

### Task 4.1: Author Release-Grade README
- **Location**: `README.md`
- **Description**: Add clear project summary, architecture map, quickstart, test instructions, and maturity disclaimer.
- **Complexity**: 6
- **Dependencies**: Task 1.1
- **Acceptance Criteria**:
  - README supports fresh clone to successful test run.
  - Includes "what Beacon is not" to avoid mis-scoped usage.
- **Validation**:
  - Fresh-environment walkthrough by non-author.

### Task 4.2: Add License and Governance Docs
- **Location**: `LICENSE`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`
- **Description**: Add foundational project/legal/governance documents.
- **Complexity**: 4
- **Dependencies**: Task 1.1
- **Acceptance Criteria**:
  - Documents match intended contribution and disclosure policy.
- **Validation**:
  - Reviewer checklist confirms consistency across docs.

### Task 4.3: Initialize Changelog and Release Notes Template
- **Location**: `CHANGELOG.md`, `docs/release/release-notes-template.md`
- **Description**: Create Keep-a-Changelog style entries and draft `v0.1.0` notes with known issues.
- **Complexity**: 3
- **Dependencies**: Sprint 2 complete
- **Acceptance Criteria**:
  - Changelog entries trace back to merged work.
- **Validation**:
  - Cross-check against git log and blocker tracker.

### Task 4.4: Add Architecture and Troubleshooting Docs
- **Location**: `docs/architecture.md`, `docs/troubleshooting.md`
- **Description**: Extract operator-focused architecture and common failure remediation from existing design docs.
- **Complexity**: 5
- **Dependencies**: Task 4.1
- **Acceptance Criteria**:
  - Docs cover setup errors, WASM/test failures, and expected debug path.
- **Validation**:
  - Dry-run troubleshooting scenarios using documented steps.

## Sprint 5: Release Candidate Execution and User Testing
**Goal**: Validate real-user readiness and close all release blockers.
**Demo/Validation**:
- RC checklist passes end-to-end.
- User testing completed with triaged outcomes.

### Task 5.1: Enforce Code Freeze Window
- **Location**: `docs/release/code-freeze-policy.md`
- **Description**: Define freeze start/end and allowed change classes during RC (for example: blocker fixes only).
- **Complexity**: 3
- **Dependencies**: Sprint 3, Sprint 4 complete
- **Acceptance Criteria**:
  - Freeze policy approved and communicated to contributors.
  - Non-allowed changes are blocked during RC.
- **Validation**:
  - Simulated non-compliant PR is rejected during freeze.

### Task 5.2: Create Repeatable RC Smoke Script
- **Location**: `scripts/release-smoke.ps1`, `docs/release/rc-runbook.md`
- **Description**: Script and document full RC gate run (`fmt`, `clippy`, `test`, docs checks, workflow status checks).
- **Complexity**: 5
- **Dependencies**: Task 5.1
- **Acceptance Criteria**:
  - RC runbook executable by any team member.
- **Validation**:
  - Independent rerun from clean clone succeeds.

### Task 5.3: Execute RC1 and Capture Evidence
- **Location**: `docs/release/rc1-evidence.md`
- **Description**: Run `G0`-`G3` gates and attach command/workflow evidence.
- **Complexity**: 3
- **Dependencies**: Task 5.2
- **Acceptance Criteria**:
  - Evidence includes command outputs and CI run links.
- **Validation**:
  - Reviewer confirms evidence integrity.

### Task 5.4: Final User Testing Plan and Sessions
- **Location**: `docs/release/user-test-plan.md`, `docs/release/user-test-results.md`
- **Description**: Define scripted tasks for first-time users; run sessions and capture friction/defects.
- **Complexity**: 6
- **Dependencies**: Task 5.3
- **Acceptance Criteria**:
  - Minimum 3 users complete scripted tasks.
  - Findings are severity-labeled and reproducible.
- **Validation**:
  - Reproduce all critical/high findings locally.

### Task 5.5: Triage, Fix, and Retest User Findings
- **Location**: `docs/release/final-triage.md`, impacted source/tests
- **Description**: Close critical/high issues, defer medium/low with explicit known-issues tracking.
- **Complexity**: 7
- **Dependencies**: Task 5.4
- **Acceptance Criteria**:
  - No unresolved critical/high issues at release gate.
  - Deferred items include mitigation and follow-up owner.
- **Validation**:
  - Re-run full RC smoke and targeted regressions.

### Task 5.6: Execute RC2 and Capture Post-Fix Evidence
- **Location**: `docs/release/rc2-evidence.md`
- **Description**: Re-run full `G0`-`G4` gates after all user-finding fixes and attach final evidence package.
- **Complexity**: 3
- **Dependencies**: Task 5.5
- **Acceptance Criteria**:
  - Final RC evidence reflects post-fix state with no open critical/high blockers.
- **Validation**:
  - Reviewer approval of RC2 evidence package.

## Sprint 6: Publish v0.1.0 and Post-Release Validation
**Goal**: Publish release safely with rollback readiness and immediate verification.
**Demo/Validation**:
- `v0.1.0` published with artifacts and release notes.
- Post-release smoke and incident response path are validated.

### Task 6.1: Final Release Cut and Tag Prep
- **Location**: `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `docs/release/release-checklist.md`
- **Description**: Freeze version metadata and finalize release checklist sign-offs.
- **Complexity**: 3
- **Dependencies**: Sprint 5 complete
- **Acceptance Criteria**:
  - Version, changelog, and release notes are consistent.
- **Validation**:
  - Final checklist has no open blocker items.

### Task 6.2: Rollback Drill Rehearsal
- **Location**: `docs/release/rollback-drill.md`
- **Description**: Rehearse rollback/hotfix path end-to-end on a rehearsal tag before GA publish.
- **Complexity**: 4
- **Dependencies**: Task 6.1
- **Acceptance Criteria**:
  - Team can cut and validate a hotfix release path from tagged baseline.
- **Validation**:
  - Drill artifact includes commands run and outcome.

### Task 6.3: Publish GitHub Release
- **Location**: GitHub Releases, `.github/workflows/release.yml`
- **Description**: Push tag and publish release with artifacts, checksums, and known issues.
- **Complexity**: 3
- **Dependencies**: Task 6.2
- **Acceptance Criteria**:
  - Release workflow green; artifacts downloadable.
- **Validation**:
  - Verify release page contents and checksums.

### Task 6.4: Post-Release Smoke and Watch
- **Location**: `docs/release/post-release-smoke.md`
- **Description**: Validate fresh clone from tag and monitor initial incoming issues for 48 hours.
- **Complexity**: 3
- **Dependencies**: Task 6.3
- **Acceptance Criteria**:
  - Post-release smoke pass recorded.
  - Hotfix criteria and escalation contacts documented.
- **Validation**:
  - Confirm ability to branch hotfix from release tag.

## Testing Strategy
- Core gate commands (must pass before merge to release branch):
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - `cargo build --workspace --locked`
- Platform matrix strategy:
  - Tier 1 (blocking): run full gate suite on `windows-latest`, `ubuntu-latest`
  - Tier 2 (informational): run at least build+test on `macos-latest`
- Add targeted regression tests for every hardening and user-test bug.
- Include deterministic replay checks for traversal-related changes.
- Include security/supply-chain checks in release runs:
  - dependency audit
  - license/dependency policy
  - secret scan
- For each sprint, require a demo artifact:
  - Sprint 1: baseline and blocker docs.
  - Sprint 2: quality gate command output.
  - Sprint 3: CI/release workflow runs.
  - Sprint 4: fresh-clone doc walkthrough result.
  - Sprint 5: user test evidence + triage closure.
  - Sprint 6: release and post-release smoke evidence.

## Potential Risks & Gotchas
- Scope ambiguity can inflate release obligations and delay final sign-off.
- Formatting-only mega-diff may obscure functional fixes during review.
- CI configuration drift can cause "works locally, fails in PR" churn.
- User-testing sample too small or too expert can hide onboarding defects.
- Deferred known issues without mitigation text can create trust risk for first release.
- Tagging before final gate evidence is complete can force avoidable hotfix release.

## Rollback Plan
- Keep release process tag-based and immutable.
- If severe issue appears after publish:
  - Mark release with advisory note.
  - Branch hotfix from release tag.
  - Patch with minimal blast radius and add regression test.
  - Publish `v0.1.1` with explicit fix notes.
- Preserve evidence docs in `docs/release/` for every RC and GA run.
