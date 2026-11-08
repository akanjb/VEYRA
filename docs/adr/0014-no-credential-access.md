# 0014. No credential access or private endpoints

- **Status:** Accepted (decision D1, approved 2026-11-08)
- **Date:** 2026-11-08

## Context

Some reference tools show live subscription quotas by reading other apps' OAuth tokens, `auth.json` files and browser cookies, then calling undocumented endpoints. That conflicts with Veyra's privacy promise and breaks without notice.

## Decision

- **Never read credentials.** Veyra never reads other applications' credential files, keychains, cookie stores or credential environment variables.
- **Enforced in code.** The connector filesystem adapter enforces a deny-list of credential files.
- **Quota only from official or first-party sources.** Quota data comes only from:
  - the Claude Code statusline JSON, forwarded by a user-installed snippet;
  - rate-limit fields written to the user's own Codex logs;
  - official OpenTelemetry;
  - official admin APIs using keys the user supplies, stored in the OS keyring.

  Everything else is shown as Unsupported, with the reason.

## Alternatives considered

- **Opt-in private endpoints:** rejected. The risk to user trust outweighs the coverage gained.

## Consequences

- Cursor, Copilot and Gemini subscription quotas are unavailable.

## Revisit when

A vendor publishes an official quota interface.
