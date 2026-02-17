# RC2 Evidence (`v0.1.0-rc2`)
RC Date: 2026-02-17
Prepared By: Codex

## Scope
RC2 re-runs gates `G0`-`G4` after blocker fixes and triage closure.

## Gate Results
| Gate | Result | Evidence Summary | Link/Ref |
| --- | --- | --- | --- |
| G0 | PASS | Security CI jobs passed on `master` and evidence is linked. | `https://github.com/ajhcs/Beacon/actions/runs/22118771571` |
| G1 | PASS | Local and CI fmt/clippy/test/build all passed on commit `08ac2bf2bbc2409cf63515e0b26b9e27f926a56e`. | `https://github.com/ajhcs/Beacon/actions/runs/22118771571` |
| G2 | IN PROGRESS | Docs exist, but docs-link validation and non-author fresh-clone validation are still pending. | `docs/release/release-checklist.md` |
| G3 | PASS | Tier 1 CI and RC tag release workflow both passed with installer verification. | `https://github.com/ajhcs/Beacon/actions/runs/22118456603` |
| G4 | NOT STARTED | Final user-validation evidence is still pending. | `docs/release/final-triage.md` |

## CI Run Tracking
| Workflow | Purpose | Run URL | Commit SHA | Status | Notes |
| --- | --- | --- | --- | --- | --- |
| `.github/workflows/ci.yml` | Tier 1 required checks + security + locked commands | `https://github.com/ajhcs/Beacon/actions/runs/22118771571` | `08ac2bf2bbc2409cf63515e0b26b9e27f926a56e` | `PASS` | `master` push run. |
| `.github/workflows/release.yml` | RC tag release packaging + installer validation | `https://github.com/ajhcs/Beacon/actions/runs/22118456603` | `5c79a309c12d3b887107fe67dabd551ea30a2c9f` | `PASS` | Triggered by tag `v0.1.0-rc1`. |

## Security Checks (CI)
| Check | Command | Run URL | Result | Notes |
| --- | --- | --- | --- | --- |
| Dependency audit | `cargo audit` | `https://github.com/ajhcs/Beacon/actions/runs/22118771571` | `PASS` | `security-cargo-audit` job. |
| Dependency policy/advisories | `cargo deny check advisories` | `https://github.com/ajhcs/Beacon/actions/runs/22118771571` | `PASS` | `security-cargo-deny` job. |
| Secret scan | `gitleaks detect --redact` | `https://github.com/ajhcs/Beacon/actions/runs/22118771571` | `PASS` | `security-gitleaks` job. |

## Installer Verification (Release CI)
| Platform | Validation Steps | Run URL | Result | Notes |
| --- | --- | --- | --- | --- |
| `ubuntu-latest` | installer script staged -> installer executed -> manifest/file checks | `https://github.com/ajhcs/Beacon/actions/runs/22118456603` | `PASS` | Build Tier 1 artifacts / linux-x86_64 job. |
| `windows-latest` | installer script staged -> installer executed -> manifest/file checks | `https://github.com/ajhcs/Beacon/actions/runs/22118456603` | `PASS` | Build Tier 1 artifacts / windows-x86_64 job. |

## Command Log (Fill During RC2)
- `cargo fmt --all -- --check`: `PASS` (local `scripts/release-smoke.ps1 -SkipSecurity`, 2026-02-17)
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: `PASS` (local `scripts/release-smoke.ps1 -SkipSecurity`, 2026-02-17)
- `cargo test --workspace --locked`: `PASS` (local `scripts/release-smoke.ps1 -SkipSecurity`, 2026-02-17)
- Local pre-RC smoke (`scripts/release-smoke.ps1 -SkipSecurity`): `PASS` on 2026-02-17
- Security scan commands: `PASS` (`cargo-audit`, `cargo-deny`, `gitleaks` in CI run `22118771571`)
- Docs validation commands: `PENDING`

## Blocker Closure Checklist
- [x] No unresolved `critical` blockers.
- [x] No unresolved `high` blockers.
- [x] Any deferred `medium`/`low` issue is documented in `docs/release/known-issues.md`.
- [x] `B-006` is closed with linked CI run evidence.

## RC2 Decision
- Outcome: `NO-GO` for GA until G2 and G4 evidence is complete.
- If `GO` later, proceed to publish gate (`G5`) and final sign-off.
