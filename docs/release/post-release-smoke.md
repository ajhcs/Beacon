# Post-Release Smoke (`v0.1.0`)
Publish Date: TBD
Owner: TBD

## Objective
Validate release integrity from public artifacts and monitor early issue signals for 48 hours.

## Immediate Checks (0-2h)
- [ ] Verify release page content, checksums, and artifact availability.
- [ ] Fresh clone from release tag succeeds.
- [ ] Core smoke command(s) run successfully from published artifacts.
- [ ] Installer flow validated on Tier 1 platforms.

## Follow-up Checks (24h/48h)
- [ ] Review incoming issues and classify by severity.
- [ ] Escalate any `critical`/`high` report to hotfix path immediately.
- [ ] Confirm no hidden regression from platform-specific reports.

## Command Record (Fill During Execution)
- Fresh clone + build/test commands:
- Artifact execution commands:
- Installer verification commands:

## Escalation
- Trigger hotfix workflow if a `critical` or reproducible `high` defect is confirmed.
- Document incident and action in this file and `docs/release/final-triage.md`.
