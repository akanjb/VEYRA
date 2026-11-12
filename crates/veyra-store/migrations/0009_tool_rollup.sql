-- 0009: tool calls per bucket, so the Tools & Agents page reads a rollup rather than
-- every call (ADR-034). Rebuilt with the other levels for every bucket a write touches,
-- and for every bucket whose calls gain an outcome; the store fills it for existing
-- data right after this migration, with the same Rust code.

-- One row per bucket, tool and dimension combination. `category` comes from the tool's
-- name (`veyra_tool_category`). `failed` counts calls reported as errors; `with_outcome`
-- counts calls whose outcome is known at all. `in_subagents` counts calls made inside
-- subagents.
CREATE TABLE rollup_15m_tools (
    bucket_start  INTEGER NOT NULL,
    slot          INTEGER NOT NULL,
    tool_name     TEXT NOT NULL,
    mcp_server    TEXT,
    category      TEXT NOT NULL,
    provider_id   TEXT,
    model_id      TEXT,
    app_id        TEXT NOT NULL,
    interface     TEXT NOT NULL,
    gateway_id    TEXT,
    project_id    INTEGER,
    device_id     INTEGER,
    calls         INTEGER NOT NULL,
    failed        INTEGER NOT NULL,
    with_outcome  INTEGER NOT NULL,
    in_subagents  INTEGER NOT NULL,
    PRIMARY KEY (bucket_start, slot)
) STRICT, WITHOUT ROWID;
