# RC Runbook (`v0.1.0`)

## Purpose
Run a repeatable release-candidate validation flow and capture evidence for gates `G0`-`G4`.

## Prerequisites
- Clean working tree for RC branch/tag.
- Access to CI workflow runs and release permissions.
- Required tools installed (`cargo`, audit/license/secret scanning tools).
- Reference checklist: `docs/release/release-checklist.md`.
- Preferred execution entrypoint: `scripts/release-smoke.ps1`.

## Execution Order
1. Confirm code freeze policy is active (`docs/release/code-freeze-policy.md`).
2. Run `G0` security checks and record outputs.
3. Run `G1` quality checks:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets --locked -- -D warnings`
   - `cargo test --workspace --locked`
4. Run `G2` docs checks (link and command validation).
5. Run `G3` automation checks (CI + release workflow on RC tag).
6. Run `G4` user-validation closure checks (for RC2).

## Evidence Capture
- RC1: update `docs/release/rc1-evidence.md` with command summaries and CI links.
- RC2: update `docs/release/rc2-evidence.md` after fixes/retests.
- Final triage updates go in `docs/release/final-triage.md`.

## Failure Handling
- Stop at first `critical`/`high` failure.
- Create or update blocker entry in `docs/release/blockers.md`.
- Re-run only after fix is merged and retested.
