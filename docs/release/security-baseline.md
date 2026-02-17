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
| Dependency audit | `cargo audit` | WIRED (`security-cargo-audit`) | PENDING (run evidence) | Job definition present in `.github/workflows/ci.yml`; no run URL captured yet. | Engineering | Push branch/tag and record first passing run URL + SHA in RC evidence docs. |
| Dependency policy/advisories | `cargo deny check advisories` | WIRED (`security-cargo-deny`) | PENDING (run evidence) | Job definition present in `.github/workflows/ci.yml`; no run URL captured yet. | Engineering | Push branch/tag and record first passing run URL + SHA in RC evidence docs. |
| Secret scan | `gitleaks detect --redact` | WIRED (`security-gitleaks`) | PENDING (run evidence) | Job definition present in `.github/workflows/ci.yml`; no run URL captured yet. | Engineering | Push branch/tag and record first passing run URL + SHA in RC evidence docs. |
| Locked mode enforcement | `cargo test --workspace --locked` (and other gate commands with `--locked`) | WIRED | PASS (definition) | Present in `.github/workflows/ci.yml` and `.github/workflows/release.yml`. | Engineering | Keep enforced; attach latest passing run URL in RC evidence docs. |
| Installer artifact path verification | Installer script execution + manifest/file output checks in release workflow | WIRED | PASS (definition) | Present in `.github/workflows/release.yml` step `Verify package installation`. | Engineering | Attach first RC tag run URL showing pass on ubuntu/windows Tier 1. |

## Open Security Blockers
- `B-006` Security baseline CI run evidence missing.

## CI Evidence Capture Template
- CI workflow run URL (security): `PENDING`
- Release workflow run URL (installer validation): `PENDING`
- Commit SHA validated: `PENDING`
- Validation date (UTC): `PENDING`

## Exit Criteria for G0
- All checks above executed with reproducible commands.
- No unresolved `critical`/`high` security findings without approved mitigation.
- Evidence copied into `docs/release/rc1-evidence.md` and `docs/release/rc2-evidence.md`.
