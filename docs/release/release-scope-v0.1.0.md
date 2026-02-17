# Release Scope v0.1.0
Date: 2026-02-17
Status: Draft for sign-off
Release Owner: TBD
Docs Reviewer: TBD

## Objective
Ship FresnelFir `v0.1.0` as the first public GitHub release with reproducible quality gates, clear platform policy, and documented limitations.

## In Scope
- Source release for current FresnelFir crates.
- Pre-built artifacts for Tier 1 platforms: `windows-latest`, `ubuntu-latest`.
- Installer build/install path for Tier 1 platforms.
- Release-program documentation under `docs/release/`.
- RC evidence package and final publish checklist.

## Out of Scope
- Net-new product features outside current mainline behavior.
- Tier 1 support commitment for `macos-latest` (Tier 2 best-effort only).
- Compatibility guarantees beyond what is explicitly documented for `v0.1.0`.
- Non-blocking performance tuning.

## Support Statement
- Tier 1 (release-blocking): `windows-latest`, `ubuntu-latest`.
- Tier 2 (best-effort): `macos-latest`.
- Tier 2 issues are triaged but are release-blocking only if severity is `critical`.

## Known Limitation at Scope Freeze
- Tier 2 (`macos-latest`) remains best-effort in `v0.1.0`; release blocking applies to Tier 1 platforms only.

## Exit Criteria
- Gates `G0` through `G5` pass in `docs/release/release-checklist.md`.
- No unresolved `critical` or `high` blockers at GA decision.

## Sign-off
- Release Owner: [ ] Approved
- Engineering Reviewer: [ ] Approved
- Docs Reviewer: [ ] Approved
