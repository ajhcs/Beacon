# Security Baseline (`v0.1.0`)
Baseline Date: 2026-02-17
Gate: `G0`

## Scope
- Dependency vulnerability status.
- Dependency policy/advisory status.
- Secret scanning status.
- Locked dependency mode usage in release validation.
- Installer artifact-path validation in release automation.

## Baseline Status
| Check | Command (example) | CI Integration | Status | Evidence | Owner | Next Action |
| --- | --- | --- | --- | --- | --- | --- |
| Dependency audit | `cargo audit` | WIRED (`security-cargo-audit`) | PASS | `https://github.com/ajhcs/Beacon/actions/runs/22118771571` (commit `08ac2bf2bbc2409cf63515e0b26b9e27f926a56e`). | Engineering | Keep as required CI check. |
| Dependency policy/advisories | `cargo deny check advisories` | WIRED (`security-cargo-deny`) | PASS | `https://github.com/ajhcs/Beacon/actions/runs/22118771571` (commit `08ac2bf2bbc2409cf63515e0b26b9e27f926a56e`). | Engineering | Keep as required CI check. |
| Secret scan | `gitleaks detect --redact` | WIRED (`security-gitleaks`) | PASS | `https://github.com/ajhcs/Beacon/actions/runs/22118771571` (commit `08ac2bf2bbc2409cf63515e0b26b9e27f926a56e`). | Engineering | Keep as required CI check. |
| Locked mode enforcement | `cargo test --workspace --locked` (and other gate commands with `--locked`) | WIRED | PASS | `https://github.com/ajhcs/Beacon/actions/runs/22118771571` and `https://github.com/ajhcs/Beacon/actions/runs/22118456603`. | Engineering | Keep enforced. |
| Installer artifact path verification | Installer script execution + manifest/file output checks in release workflow | WIRED | PASS | `https://github.com/ajhcs/Beacon/actions/runs/22118456603` (`Verify package installation` on linux/windows). | Engineering | Keep enforced in tag workflow. |

## Open Security Blockers
- None.

## CI Evidence Capture Template
- CI workflow run URL (security): `https://github.com/ajhcs/Beacon/actions/runs/22118771571`
- Release workflow run URL (installer validation): `https://github.com/ajhcs/Beacon/actions/runs/22118456603`
- Commit SHA validated: `08ac2bf2bbc2409cf63515e0b26b9e27f926a56e`
- Validation date (UTC): `2026-02-17`

## Exit Criteria for G0
- All checks above executed with reproducible commands.
- No unresolved `critical`/`high` security findings without approved mitigation.
- Evidence copied into `docs/release/rc1-evidence.md` and `docs/release/rc2-evidence.md`.
