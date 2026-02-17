# Known Issues (`v0.1.0`)
Last Updated: 2026-02-17

## Active Issues
| ID | Issue | Severity | Status | Workaround | Owner | Target |
| --- | --- | --- | --- | --- | --- | --- |
| KI-001 | Traversal path-selection TODO in `crates/beacon-explore/src/traversal/engine.rs` | high (policy-sensitive) | RESOLVED | N/A | Engineering | Closed on 2026-02-17 |
| KI-002 | Tier 2 platform (`macos-latest`) is best-effort only for `v0.1.0` | medium | OPEN | Use Tier 1 artifacts for release-critical validation | TBD | Revisit in next release planning |

## Deferral Rules
- `critical` and `high` cannot be deferred at GA.
- `medium`/`low` deferrals require:
  - mitigation text,
  - owner,
  - follow-up tracking reference.

## Notes
- Keep this file synchronized with `docs/release/blockers.md` and `docs/release/final-triage.md`.
