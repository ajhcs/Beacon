# Release Blockers
Last Updated: 2026-02-17

## Severity Taxonomy
- `critical`: Immediate risk to safety, integrity, or release correctness. Always release-blocking.
- `high`: High-confidence user or release-process failure. Release-blocking.
- `medium`: Important but can be deferred with mitigation and owner.
- `low`: Cosmetic or low-impact; does not block release.

## Blocking Policy
- RC progression (`RC1` -> `RC2`) requires no unresolved `critical`.
- GA (`v0.1.0`) requires no unresolved `critical` or `high`.
- `medium`/`low` may be deferred only if tracked in `docs/release/known-issues.md` with mitigation and owner.

## Active Blockers
| ID | Title | Severity | Gate | Owner | Status | Next Action | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- |
| B-001 | `cargo fmt --check` failing | high | G1 | Engineering | Resolved | Closed: formatter gate restored. | `docs/release/rc1-evidence.md` |
| B-002 | `cargo clippy -D warnings` failing in sandbox | high | G1 | Engineering | Resolved | Closed: strict clippy gate restored. | `docs/release/rc1-evidence.md` |
| B-003 | Missing required public docs | high | G2 | Docs | Resolved | Closed: required release-facing docs are now present. | `docs/release/rc1-evidence.md` |
| B-004 | Missing CI required checks workflow | critical | G3 | Engineering | Resolved | Closed: CI workflow is present with Tier 1 matrix and locked commands. | `.github/workflows/ci.yml` |
| B-005 | Missing release workflow | critical | G3 | Engineering | Resolved | Closed: release workflow now includes installer artifact-path validation. | `.github/workflows/release.yml` |
| B-006 | Security baseline run evidence missing | high | G0 | Engineering | In Progress | CI security jobs are wired in `.github/workflows/ci.yml`; capture first passing run URLs and SHAs in RC evidence docs, then close. | `.github/workflows/ci.yml`, `docs/release/security-baseline.md`, `docs/release/rc2-evidence.md` |
| B-007 | Traversal TODO unresolved in runtime path | high (provisional) | G1 | Engineering | Resolved | Closed: guard-failure model hash wired in traversal engine. | `docs/release/rc1-evidence.md` |

## Triage Rules
- Every blocker must have: owner, severity, reproducible evidence, and target closure gate.
- Severity changes require note in `docs/release/final-triage.md`.
- Closed blockers must reference evidence doc section proving closure.
