# 0015. Read-only snapshots for live third-party SQLite sources

- **Status:** Accepted (adopted in M4)
- **Date:** 2026-11-08

## Context

OpenCode (and later Cursor) store usage in live SQLite databases in WAL mode. Opening them with `immutable=1` can return inconsistent reads, and holding read locks can interfere with the owning application.

## Decision

- **Snapshot first.** Open the source with `mode=ro`, copy a consistent snapshot with the SQLite online backup API into a temporary file, read that snapshot, then delete it.
- **Never write.** Veyra never writes to, or holds long transactions on, a third-party database.

## Alternatives considered

- **`immutable=1`:** inconsistent reads during writes.
- **Direct long reads:** may block checkpoints of the owning application.

## Consequences

- Extra temporary disk use per scan. Large databases (Cursor `state.vscdb` can reach many GB) need size guards.

## Revisit when

The source tool offers an export or an official API.
