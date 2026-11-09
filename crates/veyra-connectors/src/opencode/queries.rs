//! The read-only queries run against a snapshot of the OpenCode database. They select
//! metadata columns and JSON fields only; message text, tool input and output are never
//! selected. Each row's `kind` is `usage` or `tool`.
//!
//! OpenCode 1.2 to 1.x keeps sessions in `session`, `message` and `part`; 2.0 added
//! `session_v2`, `session_message` and `session_part`, imported from the older tables
//! with the same message ids (the older rows stay). Both are read; the shared message id
//! makes each message count once, preferring the newer copy.

use veyra_domain::connector::DatabaseQuery;

/// Usage of assistant and compaction messages (2.0 and later). Messages a fork copied
/// from its parent (created before the fork) are left to the parent.
const USAGE_V2: DatabaseQuery = DatabaseQuery {
    tables: &["session_v2", "session_message"],
    sql: "SELECT 'usage' AS kind, 2 AS schema, m.id AS id, m.session_id AS session, \
          s.parent_id AS parent, s.directory AS directory, s.version AS version, \
          COALESCE(json_extract(m.data, '$.time.completed'), \
                   json_extract(m.data, '$.time.created'), m.time_created) AS at, \
          json_extract(m.data, '$.model.id') AS model, \
          json_extract(m.data, '$.model.providerID') AS provider, \
          json_extract(m.data, '$.tokens.input') AS input, \
          json_extract(m.data, '$.tokens.output') AS output, \
          json_extract(m.data, '$.tokens.reasoning') AS reasoning, \
          json_extract(m.data, '$.tokens.cache.read') AS cache_read, \
          json_extract(m.data, '$.tokens.cache.write') AS cache_write \
          FROM session_message m JOIN session_v2 s ON s.id = m.session_id \
          WHERE m.type IN ('assistant', 'compaction') \
            AND json_extract(m.data, '$.tokens') IS NOT NULL \
            AND (s.fork_session_id IS NULL OR m.time_created >= s.time_created)",
};

/// Tool calls stored inside assistant messages (2.0 and later, the default).
const TOOLS_V2_INLINE: DatabaseQuery = DatabaseQuery {
    tables: &["session_message"],
    sql: "SELECT 'tool' AS kind, m.id AS message, \
          json_extract(c.value, '$.id') AS call, json_extract(c.value, '$.name') AS name, \
          json_extract(c.value, '$.state.status') AS status \
          FROM session_message m, json_each(m.data, '$.content') AS c \
          WHERE m.type = 'assistant' AND json_extract(c.value, '$.type') = 'tool'",
};

/// Tool calls stored as separate rows (2.0 and later, when enabled).
const TOOLS_V2_ROWS: DatabaseQuery = DatabaseQuery {
    tables: &["session_part"],
    sql: "SELECT 'tool' AS kind, p.message_id AS message, p.call_id AS call, \
          json_extract(p.data, '$.name') AS name, \
          json_extract(p.data, '$.state.status') AS status \
          FROM session_part p WHERE p.type = 'tool'",
};

/// Usage of assistant messages (1.2 to 1.x).
const USAGE_V1: DatabaseQuery = DatabaseQuery {
    tables: &["session", "message"],
    sql: "SELECT 'usage' AS kind, 1 AS schema, m.id AS id, m.session_id AS session, \
          s.parent_id AS parent, \
          COALESCE(json_extract(m.data, '$.path.cwd'), s.directory) AS directory, \
          s.version AS version, \
          COALESCE(json_extract(m.data, '$.time.completed'), \
                   json_extract(m.data, '$.time.created'), m.time_created) AS at, \
          json_extract(m.data, '$.modelID') AS model, \
          json_extract(m.data, '$.providerID') AS provider, \
          json_extract(m.data, '$.tokens.input') AS input, \
          json_extract(m.data, '$.tokens.output') AS output, \
          json_extract(m.data, '$.tokens.reasoning') AS reasoning, \
          json_extract(m.data, '$.tokens.cache.read') AS cache_read, \
          json_extract(m.data, '$.tokens.cache.write') AS cache_write \
          FROM message m JOIN session s ON s.id = m.session_id \
          WHERE json_extract(m.data, '$.role') = 'assistant' \
            AND json_extract(m.data, '$.tokens') IS NOT NULL",
};

/// Tool calls (1.2 to 1.x).
const TOOLS_V1: DatabaseQuery = DatabaseQuery {
    tables: &["part"],
    sql: "SELECT 'tool' AS kind, p.message_id AS message, \
          json_extract(p.data, '$.callID') AS call, json_extract(p.data, '$.tool') AS name, \
          json_extract(p.data, '$.state.status') AS status \
          FROM part p WHERE json_extract(p.data, '$.type') = 'tool'",
};

/// Every query, in the order their rows are parsed.
pub const QUERIES: &[DatabaseQuery] =
    &[USAGE_V2, TOOLS_V2_INLINE, TOOLS_V2_ROWS, USAGE_V1, TOOLS_V1];
