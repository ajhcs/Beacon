---
name: beacon-verify
description: Use when the user wants to verify code against a Beacon IR specification. Governs the inner loop - compile spec, load DUT, fuzz, interpret findings, fix, re-verify. Use after a spec has been created with beacon-spec.
user_invocable: true
---

# Beacon Verification Loop

You are running the Beacon verification inner loop. Your job is to compile a formal spec, fuzz the DUT (Device Under Test), interpret findings, fix code, and re-verify until the campaign passes with zero findings and full coverage.

**Key principle:** The AI never declares success. Only Beacon (via `beacon_fuzz_status` returning `state: "complete"` with zero findings and coverage threshold met) can confirm verification.

## Prerequisites

Before starting:
1. A Beacon IR spec must exist (created via `beacon-spec` skill or manually)
2. The DUT code must exist and be compilable to WASM

## Step 1: Compile the Spec

```
beacon_compile({ "ir_json": "<contents of spec file>" })
```

**If compilation succeeds:** Note the `campaign_id` and `budget` (min iterations, timeout).

**If compilation fails:** Read the errors. Common issues:
- Dangling entity references → entity name typo in refinement
- Missing effects → action referenced in protocol but no effect defined
- Missing bindings → action without DUT binding
- Invalid repeat bounds → min > max

Fix the spec and retry. Do NOT proceed with a failed compilation.

## Step 2: Start Fuzzing

```
beacon_fuzz_start({ "campaign_id": "<id>" })
```

This transitions the campaign to running state.

## Step 3: Monitor Progress

Poll status periodically:

```
beacon_fuzz_status({ "campaign_id": "<id>" })
```

Watch for:
- `state`: "running" → still going. "complete" → done. "aborted" → something went wrong.
- `progress.iterations_done` / `progress.iterations_total` → completion percentage
- `coverage.percent` → coverage trend
- `findings_count` → findings discovered so far

## Step 4: Check Findings (Incremental)

Use incremental polling to get new findings without re-fetching all:

```
beacon_findings({ "campaign_id": "<id>", "since_seqno": <last_seqno> })
```

Start with no `since_seqno` to get all findings. Then use `next_seqno` from the response for subsequent calls.

### Interpreting Finding Types

| Type | Meaning | Action |
|------|---------|--------|
| `property_violation` | An invariant or temporal rule was violated | Fix the code logic that violates the property |
| `discrepancy` | Model prediction differs from DUT behavior | Fix DUT to match spec, or fix spec if spec was wrong |
| `crash` | DUT panicked or WASM trapped | Fix the crash — null check, bounds check, etc. |
| `timeout` | DUT action exceeded fuel/time budget | Optimize the slow code path |

### Fixing Strategy

1. **Address ALL findings, not just the first.** Multiple findings may share a root cause.
2. **Group findings by type and action** to identify patterns.
3. **Fix code, not the spec.** The spec represents approved human intent. Only modify the spec if the human explicitly approves a change.
4. **After fixing, re-compile and re-verify from Step 1.** Do not assume fixes work.

## Step 5: Coverage Check

```
beacon_coverage({ "campaign_id": "<id>" })
```

Review the coverage summary:
- `hit` targets: verified
- `pending` targets: not yet reached (may need more iterations or spec adjustment)
- `unreachable` targets: provably impossible (may indicate spec issue)

If unreachable targets exist, investigate whether the spec over-constrains the system.

## Step 6: Completion Criteria

The verification loop is complete ONLY when ALL of these are true:

1. `beacon_fuzz_status` returns `state: "complete"`
2. `beacon_findings` returns `total_findings: 0`
3. `beacon_coverage` returns `percent >= threshold` (default: 80%)

If any condition is not met, loop back to Step 4.

## Step 7: Analytics Review

After passing, optionally review analytics:

```
beacon_analytics({ "campaign_id": "<id>" })
```

Report to the user:
- Total steps executed
- Peak coverage percentage
- Finding rate per 1K steps (should be 0 at completion)
- Adaptation effectiveness
- Epochs completed

## Abort Protocol

If the campaign is stuck (no progress for extended period) or the user requests:

```
beacon_abort({ "campaign_id": "<id>" })
```

Review `final_status`, `findings_count`, and `steps_executed` to determine next steps.

## Rules

- **Never claim verification passed without checking all three completion criteria.**
- **Never modify the spec without human approval.** Fix the code instead.
- **Always re-verify after fixes.** Compile and fuzz from scratch.
- **Report findings in plain English.** Don't dump raw JSON at the user.
- **Track iteration count.** If re-verification loops exceed 5 cycles, pause and discuss with the user — the spec or approach may need revision.
