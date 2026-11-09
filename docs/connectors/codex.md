# Connector: Codex

| | |
|---|---|
| **Connector id** | `codex` |
| **Covers** | Codex CLI, the Codex IDE extension and the Codex desktop app (they share one session store) |
| **Format** | JSON Lines rollouts, optionally zstd-compressed, **undocumented** by the vendor |
| **Parser version** | 1 |
| **Verified against** | The Codex source code (`openai/codex`, commit `4f9e717`, 2026-11-09; release 0.167.0) and the desktop app's database layout on a real machine |
| **Accuracy** | Tokens, cache, reasoning: **Exact**. Requests: **Exact** (newer versions) or **Partial** (older versions). Cost: **Estimated** |
| **Code** | [`crates/veyra-connectors/src/codex`](../../crates/veyra-connectors/src/codex) |
| **Fixtures** | [`tests/fixtures/codex/v0.167`](../../tests/fixtures/codex/v0.167) (synthetic) |

## Where it reads

Root: `$CODEX_HOME` if set, else `~/.codex` (`%USERPROFILE%\.codex` on Windows).

| Path (relative to the root) | Read? |
|---|---|
| `sessions/YYYY/MM/DD/rollout-*.jsonl` | Yes |
| `archived_sessions/rollout-*.jsonl` | Yes |
| Either of the above as `.jsonl.zst` | Yes, decompressed in memory (at most 256 MiB) |
| `history.jsonl`, `auth.json`, `config.toml` | **Never** (prompt history, credentials, settings) |
| `state_*.sqlite`, `logs_*.sqlite`, everything else | **Never**. The desktop app's thread index holds only a total per thread and the first user message; the rollout files are the authoritative record |

Files are read in full whenever their size or modification time changes, because older versions record usage as running totals that depend on earlier lines.

## Line structure used

Each line is `{timestamp, type, payload}`. Only these are decoded; every content field is skipped unread.

| Line | Fields used |
|---|---|
| `session_meta` | `id`, `forked_from_id`, `parent_thread_id`, `cwd`, `originator`, `source`, `cli_version`, `model_provider` (never `base_instructions`) |
| `turn_context` | `model`, `cwd` |
| `token_usage_record` (newer versions) | `thread_id`, `response_id`, `usage` |
| `event_msg` `token_count` (older versions) | `info.total_token_usage` |
| `event_msg` `task_started`, `turn_started` | Marks a fork's first own turn |
| `response_item` `function_call`, `custom_tool_call`, `local_shell_call`, `web_search_call` | Tool `name` and `call_id` (never `arguments`, `input` or `output`) |
| `event_msg` `item_completed`, `mcp_tool_call_end`, `patch_apply_end` | Tool outcome |

## Mapping rules

| Rule | Detail |
|---|---|
| Usage source | A file with `token_usage_record` lines uses only those: one per model response, keyed `codex:<response_id>`. Otherwise usage is the increase between consecutive `token_count` totals. Both are never counted for the same file |
| Resets | A running total that goes down (Codex overwrites it after compaction or when the context window is exceeded) starts a new baseline and adds nothing |
| Repeats | A total equal to the previous one, or a `token_count` without totals (rate-limit updates), adds nothing |
| Forks | Records whose `thread_id` is not the file's thread are copies from the parent and are skipped. In older files, totals before the fork's first own turn are its baseline |
| Copies of a file | Keys depend only on the thread and its totals or response ids, so the live, archived and compressed copies of a rollout give the same records |
| Tokens | `cached_input_tokens` and `cache_write_input_tokens` are part of `input_tokens` (confirmed by Codex's own tests); uncached input = input − cached − cache writes. `reasoning_output_tokens` is part of `output_tokens` |
| Model | The latest `turn_context.model`; provider from `session_meta.model_provider` |
| Interface | Desktop app (`originator` contains "desktop"), IDE (`codex_vscode` or `source` "vscode"), otherwise CLI |
| Subagents | `parent_thread_id` or a `subagent` source links the thread to its parent session |

## Not read yet

Rate-limit snapshots (`rate_limits`) are skipped; they are planned for the quotas page.
