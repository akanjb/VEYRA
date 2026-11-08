-- 0003: leaner write path for ingestion backfills (ADR-023).

-- Re-pricing selects stale costs with `catalog_version <> ?`, which no index can serve,
-- so this index only added a B-tree write to every priced event.
DROP INDEX event_costs_catalog_version;

-- Tool calls are identified by their tool-use id. Keying the table by it (WITHOUT ROWID)
-- stores each call in one B-tree instead of a rowid table plus a unique index.
CREATE TABLE tool_calls_keyed (
    tool_use_id TEXT PRIMARY KEY,
    event_id    INTEGER NOT NULL REFERENCES usage_events (id) ON DELETE CASCADE,
    tool_name   TEXT NOT NULL,
    mcp_server  TEXT,
    mcp_tool    TEXT,
    is_error    INTEGER
) STRICT, WITHOUT ROWID;

INSERT INTO tool_calls_keyed (tool_use_id, event_id, tool_name, mcp_server, mcp_tool, is_error)
SELECT tool_use_id, event_id, tool_name, mcp_server, mcp_tool, is_error FROM tool_calls;

DROP TABLE tool_calls;
ALTER TABLE tool_calls_keyed RENAME TO tool_calls;

-- Needed by the ON DELETE CASCADE above and by per-event tool listings.
CREATE INDEX tool_calls_event ON tool_calls (event_id);
