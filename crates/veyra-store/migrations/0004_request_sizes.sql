-- 0004: request counts per size bin, for request-size distributions and percentiles
-- (ADR-026). Keyed like rollup_15m, so the same filters apply; rebuilt with it.
-- `veyra_size_bin` is registered by the store on its writer connection: the Rust
-- function is the single definition of a bin.

CREATE TABLE rollup_15m_sizes (
    bucket_start  INTEGER NOT NULL,
    slot          INTEGER NOT NULL,
    provider_id   TEXT,
    model_id      TEXT,
    app_id        TEXT NOT NULL,
    interface     TEXT NOT NULL,
    gateway_id    TEXT,
    project_id    INTEGER,
    device_id     INTEGER,
    size_bin      INTEGER NOT NULL,
    requests      INTEGER NOT NULL,
    PRIMARY KEY (bucket_start, slot)
) STRICT, WITHOUT ROWID;

-- Backfill from the stored events. The token total matches EVENT_TOTAL_TOKENS in
-- src/sql.rs at the time of this migration.
INSERT INTO rollup_15m_sizes (bucket_start, slot, provider_id, model_id, app_id, interface,
                              gateway_id, project_id, device_id, size_bin, requests)
SELECT bucket_start, ROW_NUMBER() OVER (PARTITION BY bucket_start), provider_id, model_id,
       app_id, interface, gateway_id, project_id, device_id, size_bin, COUNT(*)
FROM (
    SELECT e.*, veyra_size_bin(COALESCE(e.input_uncached, 0) + COALESCE(e.cache_read, 0)
               + COALESCE(e.cache_write_short, 0) + COALESCE(e.cache_write_long, 0)
               + COALESCE(e.cache_write_unspecified, 0) + COALESCE(e.output_total, 0)) AS size_bin
    FROM usage_events e
    WHERE e.granularity = 'request'
)
GROUP BY bucket_start, provider_id, model_id, app_id, interface, gateway_id, project_id,
         device_id, size_bin;
