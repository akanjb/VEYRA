# Connector: OpenCode

| | |
|---|---|
| **Connector id** | `opencode` |
| **Covers** | OpenCode (terminal app) |
| **Format** | SQLite database in WAL mode, owned and written live by OpenCode, **undocumented** |
| **Parser version** | 1 |
| **Verified against** | The OpenCode source code (`anomalyco/opencode`, formerly `sst/opencode`, commit `73c95ab`, 2026-11-09; release 2.2.x), including the exact table definitions |
| **Accuracy** | Tokens, cache, reasoning, requests: **Exact** (**Partial** for sessions created before 1.0.100). Cost: **Estimated** |
| **Code** | [`crates/veyra-connectors/src/opencode`](../../crates/veyra-connectors/src/opencode) |
| **Fixtures** | [`tests/fixtures/opencode/v2.2/opencode.sql`](../../tests/fixtures/opencode/v2.2/opencode.sql) (synthetic, loaded into a WAL database by the tests) |

## Where it reads

Root: `$XDG_DATA_HOME/opencode`, else `~/.local/share/opencode` on **every** operating system (on Windows, `%USERPROFILE%\.local\share\opencode`). An absolute `OPENCODE_DB` moves the root to that file's folder.

| File | Read? |
|---|---|
| `opencode.db`, `opencode-<channel>.db` | Yes, through a snapshot |
| `opencode-next.db` | No (an import source merged into `opencode.db`) |
| `storage/` (JSON files before 1.2) | No; OpenCode moves them into the database |

## How it reads (ADR-015)

The database is opened read-only and copied in one step with SQLite's backup API into a private temporary file, which is deleted right after the queries. In WAL mode this never blocks OpenCode's writers; a test keeps writing with no busy timeout while Veyra reads. The snapshot holds everything OpenCode stores, including prompts, so the queries select **only** metadata columns and JSON fields with `json_extract`, never a whole `data` column. The database is re-read when its file, or a non-empty write-ahead log, changes.

## Tables and fields used

| Tables | Generation | Fields |
|---|---|---|
| `session_v2`, `session_message` (`type` `assistant` or `compaction`) | 2.0 and later | `id`, `session_id`, `parent_id`, `fork_session_id`, `directory`, `version`, `model.id`, `model.providerID`, `tokens.*`, `time.*` |
| `session_message.content[]`, `session_part` (`type` `tool`) | 2.0 and later | Tool `id`/`call_id`, `name`, `state.status` |
| `session`, `message` (`role` `assistant`) | 1.2 to 1.x | `id`, `session_id`, `parent_id`, `directory`, `path.cwd`, `version`, `modelID`, `providerID`, `tokens.*`, `time.*` |
| `part` (`type` `tool`) | 1.2 to 1.x | `callID`, `tool`, `state.status` |

## Mapping rules

| Rule | Detail |
|---|---|
| Both generations | 2.0 imports 1.x sessions with the same message ids and keeps the old rows. Each message is keyed `opencode:<message>` and counts once; the 2.0 copy wins |
| Tokens | `input` excludes cache reads and writes. Reasoning is counted apart from `output` in 2.0 and in sessions created by 1.4 or later, and within it before 1.4 (from the session's `version`) |
| Forks | Messages a fork copied from its parent (created before the fork) are left to the parent |
| Gateways | Provider `github-copilot`, `openrouter` or `opencode` (Zen) is recorded as the gateway; the vendor comes from the model |
| Cost | OpenCode's own `cost` is mostly its estimate from a public price list, so Veyra prices the tokens itself |
| Subagents | `parent_id` links a session to its parent |

## Known gaps

Sessions older than OpenCode 1.2 that were never opened by a newer version stay in JSON files and are not read. Messages of 0.x versions that spanned several steps may show only their last step.
