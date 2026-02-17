---
name: beacon-smoke-check
description: Advisory smoke check after code modifications. Reminds the AI to re-verify when source files are written or edited.
event: PostToolUse
match_tools:
  - Write
  - Edit
---

# Beacon Smoke Check

When source code files (`.rs`, `.ts`, `.js`, `.py`, `.go`, `.wasm`, or other code files) are written or edited:

**Check if an active Beacon verification campaign exists.** If the modified file is part of a DUT (Device Under Test) that has an active campaign:

1. Note that the code has changed since the last verification run.
2. Advise: "Code modified — the current Beacon verification results may be stale. Re-compile the DUT and run `beacon_fuzz_start` to re-verify."

**This is advisory only.** Do not block the edit or automatically trigger recompilation. The verification loop skill handles the actual re-verification workflow.

**Ignore** modifications to:
- Beacon IR spec files (`.beacon.json`)
- Documentation files (`.md`)
- Configuration files (`.toml`, `.json` that aren't source)
- Test files (these don't affect DUT verification)
