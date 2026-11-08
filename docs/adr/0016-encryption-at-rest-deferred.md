# 0016. Database encryption at rest deferred

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

Veyra's database holds usage metadata (paths, project names, timestamps, token counts) but no prompts, responses or code. It lives in the per-user app data directory, protected by OS account permissions. The source transcripts themselves sit unencrypted in the same account.

## Decision

- **No application-level encryption in v1.** The threat model is a single-user OS account. Encrypting Veyra's metadata while the far more sensitive source transcripts remain plaintext adds complexity (key management, recovery) for little real protection.

## Alternatives considered

- **SQLCipher:** licensing and build complexity; key storage would rely on the OS keyring anyway.

## Consequences

- Users who need encryption at rest should use full-disk encryption (BitLocker, FileVault, LUKS). The privacy documentation says so.

## Revisit when

Organisation mode stores data from multiple users on one machine, or users request it with a concrete threat.
