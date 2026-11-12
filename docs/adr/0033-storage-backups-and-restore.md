# 0033. Storage page, daily backups and restore at start

- **Status:** Accepted
- **Date:** 2026-11-12

## Context

FR-STO-01 and FR-STO-03 ask for:
- the database's size, per-table sizes and growth;
- daily rotating backups made with `VACUUM INTO`;
- restore with a preview.

The acceptance criterion is that a restored backup round-trips.

The database cannot be swapped while Veyra runs. The writer, four readers and the ingest worker hold it open. Readers map the file into memory, and on Windows a mapped file cannot be replaced (ADR-031).

## Decision

1. **Footprint.**
   - **Per-table sizes** come from SQLite's `dbstat` table in aggregate mode, grouped by table so indexes count with their table.
   - **Event counts** come from the totals rollup. The first and last event come from the bucket index, so the page stays fast on large databases.
   - **Events per month** come from the analytics engine in the user's time zone and stand in for growth. Veyra keeps no history of its file size.

2. **Backups.**
   - **Location and names:** a backup is a `VACUUM INTO` copy in the `backups` folder beside the database, named `veyra-<UTC milliseconds>.db`. It is written under a temporary name and then renamed, so a listed backup is always complete.
   - **Retention:** the newest 7 are kept; older ones are deleted by exact name.
   - **Schedule:** a background thread takes a backup when the newest is more than a day old. It first runs ten minutes after start, so the startup scan comes first, then checks hourly. "Back up now" takes one at once.

3. **Names from the dashboard are checked.** A restore names a backup. The name must match the pattern exactly, without separators, and the file must exist in the backup folder, so a request can never reach another path.

4. **Restore is staged and applied at start.**
   - **Preview:** `preview_restore` opens the backup read-only. It refuses a schema newer than this build, and shows the backup's events and date range beside the events stored now.
   - **Restore:** `restore_backup` checks the backup the same way, copies it beside the database as `veyra.db.restore`, records the restore in the audit log and asks the shell to restart.
   - **At start:** before opening the database, the shell calls `apply_pending_restore`. It folds the old write-ahead log into the old file, keeps that file as `veyra.db.replaced` until the next restore, removes the old log files and moves the restore into place. Migrations then bring an older backup up to date.

## Alternatives considered

- **Swapping the file live.** Every connection would have to close, and the ingest worker pause and resume. Restarting is simpler and leaves nothing half open.
- **The SQLite online backup API in reverse** (writing the backup into the open database). It would rewrite the live database page by page while readers map it, and would have to stop ingestion for the duration anyway.
- **Recording the file size daily for a growth chart.** It would need a new table and a schedule. Events per month show growth in a unit people understand.

## Consequences

- A restore takes a restart. Usage recorded after the backup is read again from the tools' logs where they still exist.
- Backups use disk space: up to seven copies of the database.
- The replaced database stays on disk until the next restore, so a mistaken restore can be undone by hand.

## Revisit when

Backups need to go to another location or be encrypted (ADR-016), or the database grows large enough that a full daily copy is too slow.
