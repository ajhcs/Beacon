# RC2 Evidence (`v0.1.0-rc2`)
RC Date: TBD
Prepared By: TBD

## Scope
RC2 re-runs gates `G0`-`G4` after blocker fixes and triage closure.

## Gate Results
| Gate | Result | Evidence Summary | Link/Ref |
| --- | --- | --- | --- |
| G0 | PENDING | Awaiting CI security jobs and run evidence capture. | `docs/release/security-baseline.md` |
| G1 | PENDING | Awaiting RC2 command outputs for fmt/clippy/test. | `docs/release/release-checklist.md` |
| G2 | PENDING | Awaiting docs validation + fresh-clone validation evidence. | `docs/release/release-checklist.md` |
| G3 | PENDING | Awaiting CI and release workflow run links for Tier 1 and RC tag. | CI run links |
| G4 | PENDING | Awaiting final user-validation evidence. | `docs/release/final-triage.md` |

## CI Run Tracking
| Workflow | Purpose | Run URL | Commit SHA | Status | Notes |
| --- | --- | --- | --- | --- | --- |
| `.github/workflows/ci.yml` | Tier 1 required checks + locked commands | `PENDING` | `PENDING` | `PENDING` | Record protected-branch pass result. |
| `.github/workflows/release.yml` | RC tag release packaging + installer validation | `PENDING` | `PENDING` | `PENDING` | Record ubuntu/windows Tier 1 results. |

## Security Checks (CI)
| Check | Command | Run URL | Result | Notes |
| --- | --- | --- | --- | --- |
| Dependency audit | `cargo audit` | `PENDING` | `PENDING` | Wired as `security-cargo-audit` in `.github/workflows/ci.yml`; awaiting first run URL. |
| Dependency policy/advisories | `cargo deny check advisories` | `PENDING` | `PENDING` | Wired as `security-cargo-deny` in `.github/workflows/ci.yml`; awaiting first run URL. |
| Secret scan | `gitleaks detect --redact` | `PENDING` | `PENDING` | Wired as `security-gitleaks` in `.github/workflows/ci.yml`; awaiting first run URL. |

## Installer Verification (Release CI)
| Platform | Validation Steps | Run URL | Result | Notes |
| --- | --- | --- | --- | --- |
| `ubuntu-latest` | installer script staged -> installer executed -> manifest/file checks | `PENDING` | `PENDING` | Step definitions exist in release workflow. |
| `windows-latest` | installer script staged -> installer executed -> manifest/file checks | `PENDING` | `PENDING` | Step definitions exist in release workflow. |

## Command Log (Fill During RC2)
- `cargo fmt --all -- --check`: `PENDING`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: `PENDING`
- `cargo test --workspace --locked`: `PENDING`
- Local pre-RC smoke (`scripts/release-smoke.ps1 -SkipSecurity`): `PASS` on 2026-02-17
- Security scan commands: `PENDING`
- Docs validation commands: `PENDING`

## Blocker Closure Checklist
- [ ] No unresolved `critical` blockers.
- [ ] No unresolved `high` blockers.
- [ ] Any deferred `medium`/`low` issue is documented in `docs/release/known-issues.md`.
- [ ] `B-006` is closed with linked CI run evidence.

## RC2 Decision
- Outcome: `GO` / `NO-GO` (select one during execution).
- If `GO`, proceed to publish gate (`G5`) and final sign-off.
