-- 0001: initial schema (ADR-003, ADR-004, ADR-006, ADR-009).
-- Conventions: instants are UTC epoch milliseconds; money is integer nano-USD;
-- token columns are NULL when the source did not report them (never 0).

CREATE TABLE devices (
    id  INTEGER PRIMARY KEY,
    key TEXT NOT NULL UNIQUE
) STRICT;

CREATE TABLE projects (
    id           INTEGER PRIMARY KEY,
    key          TEXT NOT NULL UNIQUE,  -- normalised project root path
    display_name TEXT
) STRICT;

CREATE TABLE sessions (
    id                INTEGER PRIMARY KEY,
    key               TEXT NOT NULL UNIQUE,  -- source-scoped session key
    parent_session_id INTEGER REFERENCES sessions (id),
    is_subagent       INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE TABLE usage_events (
    id                      INTEGER PRIMARY KEY,
    dedup_key               TEXT NOT NULL UNIQUE,
    fingerprint             INTEGER NOT NULL,  -- detects re-ingested but unchanged events
    occurred_at             INTEGER NOT NULL,
    bucket_start            INTEGER NOT NULL,  -- 15-minute UTC bucket of occurred_at
    connector_id            TEXT NOT NULL,
    parser_version          INTEGER NOT NULL,
    tool_version            TEXT,
    session_id              INTEGER REFERENCES sessions (id),
    project_id              INTEGER REFERENCES projects (id),
    device_id               INTEGER REFERENCES devices (id),
    is_subagent             INTEGER NOT NULL,
    provider_id             TEXT,
    model_id                TEXT,
    raw_model               TEXT,
    app_id                  TEXT NOT NULL,
    interface               TEXT NOT NULL,
    gateway_id              TEXT,
    granularity             TEXT NOT NULL,
    service_tier            TEXT NOT NULL,
    inference_geo_us        INTEGER NOT NULL,
    request_count           INTEGER,
    input_uncached          INTEGER,
    cache_read              INTEGER,
    cache_write_short       INTEGER,
    cache_write_long        INTEGER,
    cache_write_unspecified INTEGER,
    output_total            INTEGER,
    reasoning               INTEGER,
    tool_tokens             INTEGER,
    reported_cost_nusd      INTEGER,
    acc_tokens              TEXT NOT NULL,
    acc_cache               TEXT NOT NULL,
    acc_reasoning           TEXT NOT NULL,
    acc_requests            TEXT NOT NULL,
    acc_cost                TEXT NOT NULL,
    ingested_at             INTEGER NOT NULL
) STRICT;

CREATE INDEX usage_events_occurred_at ON usage_events (occurred_at);
CREATE INDEX usage_events_bucket ON usage_events (bucket_start);
CREATE INDEX usage_events_session ON usage_events (session_id, occurred_at);

-- API-equivalent cost of each event, with the catalogue version that produced it.
CREATE TABLE event_costs (
    event_id                  INTEGER PRIMARY KEY REFERENCES usage_events (id) ON DELETE CASCADE,
    catalog_version           TEXT NOT NULL,
    rule_id                   TEXT,
    status                    TEXT NOT NULL,
    assumptions               INTEGER NOT NULL,
    input_nusd                INTEGER NOT NULL,
    cache_read_nusd           INTEGER NOT NULL,
    cache_write_nusd          INTEGER NOT NULL,
    output_nusd               INTEGER NOT NULL,
    total_nusd                INTEGER NOT NULL,
    cache_read_saving_nusd    INTEGER,
    cache_write_overhead_nusd INTEGER,
    rate_verified_at          TEXT
) STRICT;

CREATE INDEX event_costs_catalog_version ON event_costs (catalog_version);

-- Additive sums per 15-minute UTC bucket and dimension combination. Rebuilt from
-- usage_events for every bucket a write touches, so it can never drift. Clustered by
-- bucket (WITHOUT ROWID) so a date range is one sequential read; `slot` numbers the
-- dimension combinations within a bucket.
CREATE TABLE rollup_15m (
    bucket_start              INTEGER NOT NULL,
    slot                      INTEGER NOT NULL,
    provider_id               TEXT,
    model_id                  TEXT,
    app_id                    TEXT NOT NULL,
    interface                 TEXT NOT NULL,
    gateway_id                TEXT,
    project_id                INTEGER,
    device_id                 INTEGER,
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
    tokens_priced             INTEGER NOT NULL,  -- tokens on priced or partial events
    tokens_exact              INTEGER NOT NULL,
    tokens_provider_reported  INTEGER NOT NULL,
    tokens_partial            INTEGER NOT NULL,
    tokens_estimated          INTEGER NOT NULL,
    tokens_unsupported        INTEGER NOT NULL,
    PRIMARY KEY (bucket_start, slot)
) STRICT, WITHOUT ROWID;

-- Per-bucket totals across all dimensions (one row per bucket), rebuilt from rollup_15m.
-- Serves ungrouped, unfiltered queries such as headline numbers and long-range trends.
CREATE TABLE rollup_15m_totals (
    bucket_start              INTEGER NOT NULL PRIMARY KEY,
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
    tokens_priced             INTEGER NOT NULL,  -- tokens on priced or partial events
    tokens_exact              INTEGER NOT NULL,
    tokens_provider_reported  INTEGER NOT NULL,
    tokens_partial            INTEGER NOT NULL,
    tokens_estimated          INTEGER NOT NULL,
    tokens_unsupported        INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

-- Per-bucket sums for each single dimension (one row per bucket and dimension value),
-- rebuilt from rollup_15m. Serves queries grouped or filtered by one dimension, which
-- is most dashboard cards. Clustered by dimension, then time.
CREATE TABLE rollup_15m_by_dimension (
    dimension                 TEXT NOT NULL,     -- provider, model, app, interface, gateway, project, device
    bucket_start              INTEGER NOT NULL,
    slot                      INTEGER NOT NULL,
    value                     TEXT,              -- NULL when the attribute is unknown
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
    tokens_priced             INTEGER NOT NULL,  -- tokens on priced or partial events
    tokens_exact              INTEGER NOT NULL,
    tokens_provider_reported  INTEGER NOT NULL,
    tokens_partial            INTEGER NOT NULL,
    tokens_estimated          INTEGER NOT NULL,
    tokens_unsupported        INTEGER NOT NULL,
    PRIMARY KEY (dimension, bucket_start, slot)
) STRICT, WITHOUT ROWID;
