# Connector: Claude Code

| | |
|---|---|
| **Connector id** | `claude-code` |
| **Covers** | Claude Code CLI, the Code tab of the Claude desktop app, and the IDE extensions (they share one log format) |
| **Format** | JSON Lines, append-only, **undocumented** by the vendor (labelled as such in the app) |
| **Parser version** | 1 (bump it whenever parsing changes; files read by an older version are re-read) |
| **Observed versions** | 2.1.275 – 2.1.317 (validation spike, 2026-11-08) |
| **Accuracy** | Tokens, cache, reasoning, requests: **Exact**. Cost: **Estimated** (API-equivalent) |
| **Code** | [`crates/veyra-connectors/src/claude_code`](../../crates/veyra-connectors/src/claude_code) |
| **Fixtures** | [`tests/fixtures/claude-code/v2.1`](../../tests/fixtures/claude-code/v2.1) (synthetic) |

## Where it reads

The root is `~/.claude/projects`, or `$CLAUDE_CONFIG_DIR/projects` when the documented `CLAUDE_CONFIG_DIR` variable relocates the configuration directory. No other environment variable is consulted (ADR-014).

Inside the root, the connector reads **exactly two shapes of path**. Everything else is ignored, including tool-output files, images, memory notes and any credential file:

```text
projects/
└── <project>/                         # the working directory, with separators replaced
    ├── <session-id>.jsonl             # ✔ main session log
    └── <session-id>/
        ├── subagents/
        │   └── <agent>.jsonl          # ✔ subagent (sidechain) log
        ├── tool-results/…             # ✘ never read
        └── …                          # ✘ never read
```

Files are opened read-only, with full sharing on Windows, so Claude Code can keep appending, renaming or deleting them. Symbolic links are never followed.

## Validation spike findings

The spike read structure only (field names, types and counts) from the local logs of the machine that built M2. No content was printed or stored. Findings:

| Observation | Value |
|---|---|
| Files | 249 (25 main sessions, 224 subagent logs) |
| Lines carrying `usage` | about 118,000 |
| Distinct API requests | 38,364 |
| Copies of one request | up to 40 lines (streaming writes one line per content block) |
| Requests whose copies disagree | 3,565. In every case, the line with a `stop_reason` and the highest token total is the final one |
| Requests spanning several files of one session | 1,378 |
| `thinking_tokens` greater than `output_tokens` | 42 lines, all superseded copies |
| Cache-write split not summing to the total | 47 lines, all superseded copies |
| Multi-iteration usage lines | none |
| `entrypoint` values | `claude-desktop` on this machine |

Conclusions:
- One request is one event.
- The most complete copy wins.
- `thinking_tokens` is a subset of `output_tokens`.
- Anomalous values are never used.

## Line structure used

Only these fields are deserialised. Text is skipped without buffering (a custom visitor), so prompt, response, thinking and tool text never enter memory as strings.

| Field | Use |
|---|---|
| `requestId` (fallback `message.id`) | Natural key `anthropic:<requestId>` |
| `timestamp` | Event time (UTC) |
| `sessionId`, `agentId`, `isSidechain` | Session key `claude-code:<session>[:<agent>]`; subagents link to their parent session |
| `cwd` | Project key (normalised path) |
| `version`, `entrypoint` | Tool version; interface |
| `message.model` | Model, resolved through the registry (aliases included) |
| `message.stop_reason` | Completeness (see precedence) |
| `message.content[]` of type `tool_use` | Tool call **id and name only** (never `input`) |
| `message.content[]` of type `tool_result` | Outcome: `tool_use_id` and `is_error` (never `content`) |
| `message.usage.*` | Token counts, service tier, inference region, speed |

## Mapping rules

| Canonical field | Source | Rule |
|---|---|---|
| `input_uncached` | `usage.input_tokens` | As is |
| `cache_read` | `usage.cache_read_input_tokens` | As is |
| `cache_write_short` / `cache_write_long` | `usage.cache_creation.ephemeral_5m_input_tokens` / `ephemeral_1h_input_tokens` | Used only when they sum to `cache_creation_input_tokens`; otherwise the total goes to `cache_write_unspecified` and the TTL split is left unknown |
| `output_total` | `usage.output_tokens` | Includes thinking |
| `reasoning` | `usage.output_tokens_details.thinking_tokens` | Left unknown, with reasoning accuracy **Partial**, when larger than the output total |
| `requests` | — | 1 per request |
| Service tier | `usage.speed`, `usage.service_tier` | `speed = fast` → Fast; otherwise `batch` / `priority` / `flex` / Standard |
| US-only inference | `usage.inference_geo` | `us` → regional multiplier applies |
| Interface | `entrypoint` | `claude-desktop` → Desktop; `vscode`, `jetbrains`, `ide…` → IDE; `sdk…` → API; otherwise CLI |
| Provider | — | `anthropic` |

Lines whose model is `<synthetic>` are local placeholders, not API calls, and are skipped.

## Duplicates and precedence

Each observation carries a **precedence**: a high bit when the line has a `stop_reason`, plus the token total. Within a parse, the connector keeps the highest-precedence copy of each request and merges the tool calls of all copies. Across chunks and files, the store applies the same rule: a lower precedence never replaces a higher one, and on a tie the earlier timestamp wins. The result is therefore **independent of reading order**, which the store tests prove by writing observations in every order.

## OpenTelemetry events

With the local receiver turned on (ADR-038), Claude Code can push its documented `claude_code.api_request` log event instead of, or in addition to, the log files being read. Set these where Claude Code runs:

```text
CLAUDE_CODE_ENABLE_TELEMETRY=1
OTEL_LOGS_EXPORTER=otlp
OTEL_EXPORTER_OTLP_LOGS_PROTOCOL=http/json
OTEL_EXPORTER_OTLP_LOGS_ENDPOINT=http://127.0.0.1:4318/v1/logs
```

| Attribute | Use |
|---|---|
| `request_id` | Natural key `anthropic:<request_id>`, the same as the log line |
| `event.timestamp` | Time of the request (the record time when absent) |
| `model` | Model, resolved through the registry |
| `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_creation_tokens` | Token counts, with the same meaning as the API usage block |
| `cache_creation_5m_tokens`, `cache_creation_1h_tokens` | Cache-write TTL split; kept only when it adds up to the total |
| `speed` | `fast` prices at the Fast tier |
| `session.id`, `agent_id` | Session, and subagent session under it |
| `app.entrypoint` | Interface, when Claude Code is set to include it |
| `service.version` | Claude Code version |

Every other attribute is ignored, including `cost_usd` (Claude Code's own estimate), account and email identifiers, and any prompt or tool content. The event has no stop reason, so it ranks below a completed log line: whichever arrives first, the stored request ends up as the log line describes it.

## Diagnostics

Problems are counted per file and kind in `ingest_issues`, with the byte offset of the first occurrence. Content is never recorded.

| Kind | Meaning |
|---|---|
| `malformed-json` | A line that could carry usage is not valid JSON (for example, a truncated write) |
| `missing-request-id` | A usage line with neither `requestId` nor `message.id` |
| `invalid-timestamp` | A usage line without a parseable timestamp |
| `invalid-record` | Values violate a domain invariant |
| `oversize-line` | A single line longer than the read chunk (8 MiB); skipped |
| `unreadable-file` | The file vanished or could not be opened during the scan |

## Real-data check

Run on the build machine on 2026-11-08 with the developer diagnostic (`cargo run --release -p veyra-ingest --example scan_root -- <projects-dir> <new-db-file>`), which prints counts only:

| | |
|---|---|
| Input | 249 files, 1.3 GB, 332,828 lines |
| Result | 38,509 events, 100% priced, 0 issues |
| Duplicates resolved | 11,198 identical copies, 140 upgrades, 52 superseded copies |
| First scan | 8.9 s (memory-constrained machine) |
| Rescan | 0.2 s, no changes |
| Canary | the logs contain `VEYRA-CANARY` strings; the database, WAL and shared-memory files contain none |

## When the format changes

1. Run the diagnostic against the new version and compare issue counts.
2. Add a fixture directory for the new format version (`tests/fixtures/claude-code/v<version>`). Use synthetic content only, with canary markers wherever real logs hold content.
3. Adjust the parser, bump `PARSER_VERSION` and update this page. Changed files are re-read automatically, and unchanged results are not rewritten.
