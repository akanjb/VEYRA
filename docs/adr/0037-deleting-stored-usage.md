# 0037. Deleting stored usage

- **Status:** Accepted
- **Date:** 2026-11-19

## Context

FR-STO-04 asks that people can delete Veyra's own data: all of it, one application's, or a date range, with confirmation, and never the files of the tools Veyra reads.

Every page reads the rollups (ADR-022, ADR-030), which are derived from the stored events. Deleting events without rebuilding their buckets would leave pages showing usage that no longer exists.

## Decision

1. **Scope.** A deletion names an optional application (its connector), an optional first local day and an optional last local day. Every part left out widens the scope; all left out means every event. Local midnights fall on 15-minute bucket edges, so the days are matched on the bucket, which the bucket index serves.
2. **Preview first.** `preview_delete` counts the events the scope covers and their first and last time, before anything changes. The dashboard shows it as the scope is edited and again in the confirmation.
3. **One transaction.** `delete_data` deletes the events; their costs and tool calls go with them (`ON DELETE CASCADE`). Every rollup level is rebuilt for the buckets the deleted events were in, in the same transaction, so no page ever shows a half-deleted state.
4. **Backed up first.** Before deleting, Veyra takes a backup (ADR-033), so a mistaken deletion can be undone by restoring it. The deletion is recorded in the audit log with the number of events only.
5. **Read positions are kept.** Each file's read position stays, so the deleted lines are not read again. New usage keeps arriving from sources that stay enabled.

## Consequences

- Sessions and projects are kept: pages reach them only through events, so a session with no events left is not shown, and project names, merges and budgets survive.
- If a tool replaces a file whose events were deleted, Veyra reads it again from the start, as it does for any replaced file, and those events return.
- At ten million events, deleting everything rebuilds every bucket, which takes as long as the first full import's rollup step.
