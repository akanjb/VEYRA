# Connector: Gemini CLI

| | |
|---|---|
| **Connector id** | `gemini-cli` |
| **Covers** | Google Gemini CLI |
| **Format** | JSON Lines (v0.39 and later) or one JSON document per session (earlier), **undocumented** |
| **Parser version** | 1 |
| **Verified against** | The Gemini CLI source code (`google-gemini/gemini-cli`, commit `a7bf415`, 2026-11-09; release 0.67.0) |
| **Accuracy** | Tokens, cache, reasoning, requests: **Exact** for the requests recorded. Cost: **Estimated** |
| **Code** | [`crates/veyra-connectors/src/gemini_cli`](../../crates/veyra-connectors/src/gemini_cli) |
| **Fixtures** | [`tests/fixtures/gemini-cli/v0.67`](../../tests/fixtures/gemini-cli/v0.67) (synthetic) |

## Where it reads

Root: `~/.gemini/tmp` (`GEMINI_CLI_HOME` replaces the home directory). The macOS sandbox uses `~/.cache/.gemini/tmp`, which is also checked.

| Path (relative to the root) | Read? |
|---|---|
| `<project>/chats/session-*.jsonl` | Yes |
| `<project>/chats/session-*.json` (before v0.39) | Yes |
| `<project>/chats/<session>/<agent>.jsonl` (subagents) | Yes |
| `<project>/.project_root` | Yes, as context only: the project folder (at most 4 KiB) |
| `<project>/logs.json`, `checkpoint-*.json`, `checkpoints/`, `*.tmp-*`, `*.unreadable-*` | **Never** (prompt logs, saved conversations, temporary files) |

`<project>` is a readable folder name since v0.29 and a SHA-256 hash before; both are accepted. Files are read in full whenever they change, because a message is written again each time it changes.

## Record structure used

| Record | Fields used |
|---|---|
| Metadata (first line) | `sessionId`, `kind` (`subagent`), `directories` |
| Message | `id`, `type`, `timestamp`, `model`, `tokens`, `toolCalls[].id`, `name`, `status` (never `content`, `thoughts`, `args` or `result`) |
| `$set` | `sessionId` (resumed sessions), `messages` (old checkpoints) |
| Document (before v0.39) | `sessionId`, `messages` |

## Mapping rules

| Rule | Detail |
|---|---|
| One count per message | The last copy of each message id counts, once, keyed `gemini:<session>:<message>`. A resumed old document and its new copy share these keys |
| Rewinds | Messages removed by `$rewindTo` still count: their tokens were spent |
| Tokens | `input` includes `cached`; tool-use prompt tokens (`tool`) are billed as input but reported apart, so uncached input = input − cached + tool. `thoughts` are reasoning, reported apart from `output`, so output = output + thoughts. This matches Gemini's own `total` |
| Missing model | Usage can attach to a message without a model; it takes the nearest model in the session |
| Project | From `.project_root`, else the first of `directories` |
| Subagents | Files in `chats/<session>/` link to that parent session |

## Known gaps

Gemini CLI records only conversation requests. Background requests, such as history compression and routing, are not written to these files, so totals can be below what Google bills. The Sources page states this.
