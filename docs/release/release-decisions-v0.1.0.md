# Release Decisions v0.1.0
Decision Freeze Date: 2026-02-17
Applies To: `v0.1.0` only

## Fixed Decisions

### D-001 Artifact Scope (Fixed)
- Decision: Publish `source + pre-built artifacts + installer`.
- Rationale: First release needs both developer and operator entry paths.
- Impact: Release workflow must produce and validate artifacts and installer.

### D-002 Platform Policy (Fixed)
- Decision:
  - Tier 1 (release-blocking): `windows-latest`, `ubuntu-latest`
  - Tier 2 (best-effort): `macos-latest`
- Rationale: Focus blocking support where test confidence and usage are highest.
- Impact: CI branch protection requires Tier 1 green; Tier 2 remains informational unless `critical`.

### D-003 TODO Policy (Fixed)
- Decision:
  - No unresolved TODOs in release-critical runtime paths at GA.
  - TODOs allowed only in tests/docs if linked to tracked issue and non-blocking.
- Rationale: Avoid shipping known runtime correctness debt in first public release.
- Impact: Traversal TODO must be resolved or explicitly deferred with mitigation and approval.

## Change Control
- These decisions are locked for `v0.1.0`.
- Any exception requires explicit approval by release owner and update in `docs/release/final-triage.md`.
