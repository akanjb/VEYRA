-- 0008: stored settings and the audit log (ADR-032).
--
-- A setting is a JSON value under a stable key. The audit log records what the user did
-- that takes data out of Veyra or changes it, such as an export. Its details never hold
-- paths, project names or file names.

CREATE TABLE settings (
    key        TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL                  -- UTC milliseconds
) STRICT, WITHOUT ROWID;

CREATE TABLE audit_log (
    id     INTEGER PRIMARY KEY,
    at     INTEGER NOT NULL,                     -- UTC milliseconds
    actor  TEXT NOT NULL CHECK (actor IN ('user', 'system')),
    action TEXT NOT NULL,
    detail TEXT NOT NULL
) STRICT;
