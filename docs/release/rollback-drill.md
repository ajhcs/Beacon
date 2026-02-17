# Rollback Drill (`v0.1.0`)
Drill Date: TBD
Facilitator: TBD

## Goal
Prove the team can recover from a bad release by issuing a minimal hotfix from the release baseline.

## Drill Steps
1. Select rehearsal tag baseline (for example `v0.1.0-rc2`).
2. Simulate release issue declaration and advisory note.
3. Create hotfix branch from rehearsal tag.
4. Apply minimal patch and add/verify regression test.
5. Run release smoke on hotfix candidate.
6. Cut rehearsal hotfix tag (for example `v0.1.1-rc-hotfix`) and validate artifacts.

## Evidence to Capture
- Commands executed.
- CI/release workflow links.
- Time to advisory, time to candidate hotfix, time to validated hotfix.
- Any process gaps discovered.

## Success Criteria
- Hotfix path is executable without ad-hoc process invention.
- Evidence confirms reproducible fix and validation.
- Follow-up actions are recorded before GA publish.
