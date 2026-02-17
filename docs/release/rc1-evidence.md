# RC1 Evidence (`v0.1.0-rc1`)
RC Date: TBD
Prepared By: TBD

## Scope
RC1 covers gates `G0`-`G3` and captures blocker posture before final user-validation closure.

## Gate Results
| Gate | Result | Evidence Summary | Link/Ref |
| --- | --- | --- | --- |
| G0 | IN PROGRESS | Security scans are wired in CI, but run URLs/SHAs are still pending; blocker `B-006` remains open. | `docs/release/security-baseline.md`, `.github/workflows/ci.yml` |
| G1 | PASS (local) | Local release smoke passed fmt/clippy/tests/build with locked mode on 2026-02-17. | `scripts/release-smoke.ps1`, `docs/release/release-checklist.md` |
| G2 | IN PROGRESS | Required release docs now exist; docs-validation and fresh-clone validation evidence still pending. | `docs/release/release-checklist.md` |
| G3 | IN PROGRESS | CI and release workflows are present; installer-path validation is defined in release workflow. Passing run URLs are pending. | `.github/workflows/ci.yml`, `.github/workflows/release.yml` |

## Workflow Definition Evidence (No Run URLs Yet)
- CI workflow path: `.github/workflows/ci.yml`
- Release workflow path: `.github/workflows/release.yml`
- Installer validation steps present:
- `Stage installer scripts`
- `Verify package installation`

## Command Log (Fill During RC1)
- `cargo fmt --all -- --check`: `PASS` (local, 2026-02-17 via `scripts/release-smoke.ps1 -SkipSecurity`)
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: `PASS` (local, 2026-02-17 via `scripts/release-smoke.ps1 -SkipSecurity`)
- `cargo test --workspace --locked`: `PASS` (local, 2026-02-17 via `scripts/release-smoke.ps1 -SkipSecurity`)
- Security scan commands: `PENDING CI RUN URL` (jobs now wired in `.github/workflows/ci.yml`)

## Open Blockers After RC1
- [x] B-001
- [x] B-002
- [x] B-003
- [x] B-004
- [x] B-005
- [ ] B-006
- [x] B-007

## RC1 Decision
- Outcome: `NO-GO` until blocker `B-006` is closed and command/run evidence is attached.
- Required next step: run CI/release workflows on GitHub and attach run URLs + SHAs in RC2 evidence.
