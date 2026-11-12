-- 0007: rollups that keep every dashboard page within its budget at ten million events
-- (ADR-030). Rebuilt with the other levels for every bucket a write touches; the store
-- fills them for existing data right after this migration, with the same Rust code.

-- One row per session, bucket, project, application and interface: the session's first
-- and last request in the bucket, its active intervals there (`veyra_active_intervals`,
-- as `start:end` pairs), its distinct models and the usual sums.
CREATE TABLE rollup_15m_sessions (
    bucket_start              INTEGER NOT NULL,
    slot                      INTEGER NOT NULL,
    session_id                INTEGER NOT NULL,
    project_id                INTEGER,
    app_id                    TEXT NOT NULL,
    interface                 TEXT NOT NULL,
    first_at                  INTEGER NOT NULL,
    last_at                   INTEGER NOT NULL,
    active                    TEXT NOT NULL,
    models                    TEXT,
    events                    INTEGER NOT NULL,
    requests                  INTEGER,
    input_uncached            INTEGER,
    cache_read                INTEGER,
    cache_write_short         INTEGER,
    cache_write_long          INTEGER,
    cache_write_unspecified   INTEGER,
    output_total              INTEGER,
    reasoning                 INTEGER,
    tool_tokens               INTEGER,
    reported_cost_nusd        INTEGER,
    cost_input_nusd           INTEGER NOT NULL,
    cost_cache_read_nusd      INTEGER NOT NULL,
    cost_cache_write_nusd     INTEGER NOT NULL,
    cost_output_nusd          INTEGER NOT NULL,
    cost_total_nusd           INTEGER NOT NULL,
    cache_read_saving_nusd    INTEGER,
    cache_write_overhead_nusd INTEGER,
    events_priced             INTEGER NOT NULL,
    events_partial            INTEGER NOT NULL,
    events_unpriced           INTEGER NOT NULL,
    tokens_total              INTEGER NOT NULL,
    tokens_priced             INTEGER NOT NULL,
    tokens_exact              INTEGER NOT NULL,
    tokens_provider_reported  INTEGER NOT NULL,
    tokens_partial            INTEGER NOT NULL,
    tokens_estimated          INTEGER NOT NULL,
    tokens_unsupported        INTEGER NOT NULL,
    PRIMARY KEY (bucket_start, slot)
) STRICT, WITHOUT ROWID;

CREATE INDEX rollup_15m_sessions_session ON rollup_15m_sessions (session_id, bucket_start);

-- Sums per bucket and provider, model, application and interface: answers queries that
-- group or filter by several of these without reading every dimension combination.
CREATE TABLE rollup_15m_services (
    bucket_start              INTEGER NOT NULL,
    slot                      INTEGER NOT NULL,
    provider_id               TEXT,
    model_id                  TEXT,
    app_id                    TEXT NOT NULL,
    interface                 TEXT NOT NULL,
    events                    INTEGER NOT NULL,
    requests                  INTEGER,
    input_uncached            INTEGER,
    cache_read                INTEGER,
    cache_write_short         INTEGER,
    cache_write_long          INTEGER,
    cache_write_unspecified   INTEGER,
    output_total              INTEGER,
    reasoning                 INTEGER,
    tool_tokens               INTEGER,
    reported_cost_nusd        INTEGER,
    cost_input_nusd           INTEGER NOT NULL,
    cost_cache_read_nusd      INTEGER NOT NULL,
    cost_cache_write_nusd     INTEGER NOT NULL,
    cost_output_nusd          INTEGER NOT NULL,
    cost_total_nusd           INTEGER NOT NULL,
    cache_read_saving_nusd    INTEGER,
    cache_write_overhead_nusd INTEGER,
    events_priced             INTEGER NOT NULL,
    events_partial            INTEGER NOT NULL,
    events_unpriced           INTEGER NOT NULL,
    tokens_total              INTEGER NOT NULL,
    tokens_priced             INTEGER NOT NULL,
    tokens_exact              INTEGER NOT NULL,
    tokens_provider_reported  INTEGER NOT NULL,
    tokens_partial            INTEGER NOT NULL,
    tokens_estimated          INTEGER NOT NULL,
    tokens_unsupported        INTEGER NOT NULL,
    PRIMARY KEY (bucket_start, slot)
) STRICT, WITHOUT ROWID;

-- How events were priced, per bucket, provider, model id and reported model name, price
-- rule, outcome and set of assumptions: the cost assumptions panel.
CREATE TABLE rollup_15m_pricing (
    bucket_start    INTEGER NOT NULL,
    slot            INTEGER NOT NULL,
    provider_id     TEXT,
    model_id        TEXT,
    raw_model       TEXT,
    rule_id         TEXT,
    status          TEXT NOT NULL,
    assumptions     INTEGER NOT NULL,
    events          INTEGER NOT NULL,
    tokens          INTEGER NOT NULL,
    cost_total_nusd INTEGER NOT NULL,
    PRIMARY KEY (bucket_start, slot)
) STRICT, WITHOUT ROWID;

-- Requests per bucket and size bin across every dimension: request sizes without
-- filters, read from far fewer rows than rollup_15m_sizes.
CREATE TABLE rollup_15m_size_totals (
    bucket_start INTEGER NOT NULL,
    size_bin     INTEGER NOT NULL,
    requests     INTEGER NOT NULL,
    PRIMARY KEY (bucket_start, size_bin)
) STRICT, WITHOUT ROWID;
