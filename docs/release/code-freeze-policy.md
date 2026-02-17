# Code Freeze Policy (`v0.1.0`)
Effective: from `RC1` tag creation until GA publish decision

## Freeze Objective
Stabilize the release candidate by limiting changes to blocker resolution and release safety work.

## Allowed During Freeze
- Fixes for `critical`/`high` blockers.
- Minimal-risk test additions proving blocker fixes.
- Documentation corrections required for gate completion.
- Release pipeline/configuration fixes required to clear `G0`-`G5`.

## Not Allowed During Freeze
- New features.
- Non-essential refactors.
- Broad dependency upgrades unrelated to blocker/security remediation.
- Cosmetic churn that increases review surface without gate value.

## Exception Process
1. Request exception with rationale, risk, and rollback plan.
2. Require release owner approval before merge.
3. Record approved exception in `docs/release/final-triage.md`.

## Merge Rules in Freeze Window
- Two reviewers on blocker fixes (engineering + release owner/delegate).
- All affected required checks must be green before merge.
- Every merge must reference blocker ID(s) and evidence update.
