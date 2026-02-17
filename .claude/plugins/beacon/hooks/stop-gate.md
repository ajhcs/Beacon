---
name: beacon-stop-gate
description: Quality gate that blocks task completion unless Beacon verification has passed. Prevents the AI from declaring work complete without evidence.
event: Stop
---

# Beacon Verification Stop Gate

Before allowing the AI to stop or declare a task complete, verify that Beacon verification requirements are met.

**This gate applies when:**
- A Beacon IR spec exists for the current project
- Code changes were made during this session that affect the DUT

**Gate conditions — ALL must be true:**

1. **Active campaign exists.** Check via `beacon_status` — `active_campaigns > 0`.
2. **Campaign is complete.** Check via `beacon_fuzz_status` — `state == "complete"`.
3. **Zero findings.** Check via `beacon_findings` — `total_findings == 0`.
4. **Coverage threshold met.** Check via `beacon_coverage` — `percent >= 80` (configurable).

**If any condition fails:**

Report to the AI which conditions are not met:

- No active campaign: "No Beacon verification campaign is active. Run the verification loop before completing."
- Campaign not complete: "Beacon campaign is still running (N/M iterations). Wait for completion or abort."
- Findings exist: "Beacon found N issues. Address all findings before completing."
- Coverage below threshold: "Beacon coverage is at X%, below the 80% threshold."

**The AI should then:**
1. Address the failing conditions (run verification, fix findings, etc.)
2. Re-attempt completion only after all conditions pass

**Gate bypass:**
- If the user explicitly says to skip verification (e.g., "skip beacon", "just commit"), allow it but note: "Proceeding without Beacon verification as requested."
- If no Beacon spec exists for this project, the gate does not apply.
