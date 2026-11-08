-- 0002: incremental ingestion (ADR-023).

-- How complete each stored observation is: a lower-precedence observation of the same
-- event never replaces a higher one, so results do not depend on read order.
ALTER TABLE usage_events ADD COLUMN precedence INTEGER NOT NULL DEFAULT 0;

-- No query filters raw events by time (analytics reads the rollups, and rebuilds select
-- by bucket), so this index only slowed every insert.
DROP INDEX usage_events_occurred_at;

-- Usage sources found on this device. Disabled until the user consents (FR-ING-02).
CREATE TABLE sources (
    id              INTEGER PRIMARY KEY,
    connector_id    TEXT NOT NULL,
    root            TEXT NOT NULL,
    enabled         INTEGER NOT NULL DEFAULT 0,
    consented_at    INTEGER,
    discovered_at   INTEGER NOT NULL,
    last_scan_at    INTEGER,
    last_success_at INTEGER,
    last_error      TEXT,
    UNIQUE (connector_id, root)
) STRICT;

-- Per-file read checkpoints. `head_hash` fingerprints the first `head_len` bytes so a
-- replaced file is detected even when it grows past the old offset.
CREATE TABLE source_files (
    id               INTEGER PRIMARY KEY,
    source_id        INTEGER NOT NULL REFERENCES sources (id) ON DELETE CASCADE,
    path             TEXT NOT NULL,     -- relative to the source root
    size             INTEGER NOT NULL,
    modified_at      INTEGER,
    byte_offset      INTEGER NOT NULL,
    head_hash        INTEGER NOT NULL,
    head_len         INTEGER NOT NULL,
    parser_version   INTEGER NOT NULL,
    state            TEXT NOT NULL,     -- active | missing
    last_ingested_at INTEGER NOT NULL,
    UNIQUE (source_id, path)
) STRICT;

CREATE TABLE ingest_runs (
    id               INTEGER PRIMARY KEY,
    trigger          TEXT NOT NULL,
    started_at       INTEGER NOT NULL,
    finished_at      INTEGER,
    files_read       INTEGER NOT NULL DEFAULT 0,
    lines_read       INTEGER NOT NULL DEFAULT 0,
    events_inserted  INTEGER NOT NULL DEFAULT 0,
    events_updated   INTEGER NOT NULL DEFAULT 0,
    events_unchanged INTEGER NOT NULL DEFAULT 0,
    issues           INTEGER NOT NULL DEFAULT 0,
    outcome          TEXT
) STRICT;

-- Recoverable parse problems, counted per file and kind. Never contains source content.
CREATE TABLE ingest_issues (
    run_id       INTEGER NOT NULL REFERENCES ingest_runs (id) ON DELETE CASCADE,
    source_id    INTEGER NOT NULL REFERENCES sources (id) ON DELETE CASCADE,
    path         TEXT NOT NULL,
    kind         TEXT NOT NULL,
    occurrences  INTEGER NOT NULL,
    first_offset INTEGER NOT NULL,
    PRIMARY KEY (run_id, source_id, path, kind)
) STRICT;

-- Tool invocations as metadata only: name, MCP server/tool split and outcome.
CREATE TABLE tool_calls (
    id          INTEGER PRIMARY KEY,
    event_id    INTEGER NOT NULL REFERENCES usage_events (id) ON DELETE CASCADE,
    tool_use_id TEXT NOT NULL UNIQUE,
    tool_name   TEXT NOT NULL,
    mcp_server  TEXT,
    mcp_tool    TEXT,
    is_error    INTEGER
) STRICT;

CREATE INDEX tool_calls_event ON tool_calls (event_id);
