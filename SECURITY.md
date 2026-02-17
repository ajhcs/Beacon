# Security Policy

## Supported scope

FresnelFir is pre-1.0 and currently developed on `main`.

- `main`: supported for security fixes.
- Historical commits and unpublished snapshots: best effort only.
- No compatibility or patch guarantees are made for untagged past revisions.

## Reporting a vulnerability

Please report suspected vulnerabilities privately.

1. Preferred: use your repository host's private vulnerability reporting flow (for GitHub, "Security" -> "Report a vulnerability").
2. If private reporting is unavailable, open an issue with minimal details and request a secure contact channel. Do not publish exploit details.
3. Include:
- affected crate/module
- reproducible steps or proof-of-concept
- impact assessment
- possible mitigation (if known)

## Response process

- We will acknowledge receipt as soon as maintainers are available.
- We will validate the report, assess severity, and coordinate a fix.
- Public disclosure should wait until a fix or mitigation is available.

## Out of scope

- Reports that only cover unsupported historical snapshots.
- Purely theoretical issues without a credible attack path or reproduction.
