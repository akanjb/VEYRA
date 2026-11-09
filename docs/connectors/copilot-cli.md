# Connector: GitHub Copilot CLI

| | |
|---|---|
| **Connector id** | `copilot-cli` |
| **Covers** | GitHub Copilot CLI (`@github/copilot`) |
| **Format** | JSON Lines session events; the event schema is published with the Copilot SDK, the files themselves are not documented |
| **Parser version** | 1 |
| **Verified against** | The session event schema (`github/copilot-sdk`, 2026-11-07), the CLI changelog, and the CLI's shipped code for 0.0.350 to 1.0.80 |
| **Accuracy** | Tokens and requests: **Provider-reported**, per session run. Cache: **Partial**. Cost: **Estimated** (Copilot bills premium requests) |
| **Granularity** | One record per session run and model, not per request |
| **Code** | [`crates/veyra-connectors/src/copilot_cli`](../../crates/veyra-connectors/src/copilot_cli) |
| **Fixtures** | [`tests/fixtures/copilot-cli/v1.0`](../../tests/fixtures/copilot-cli/v1.0) (synthetic) |

## Where it reads

Root: `$COPILOT_HOME` if set, else `~/.copilot`.

| Path (relative to the root) | Read? |
|---|---|
| `session-state/<session>/events.jsonl` | Yes |
| `session-state/<session>.jsonl` (0.0.342 to 0.0.379) | Yes |
| `logs/` | **Never** (debug logs; before 1.0.87 they could contain secrets from the shell) |
| `config.json`, `session-store.db`, `history-session-state/`, `workspace.yaml` | **Never** |

Files are read in full whenever they change, because how a summary counts depends on the summaries before it.

## Why usage is per session run

Copilot CLI writes no per-request usage to disk in any version: the per-request event (`assistant.usage`) exists only while the CLI runs. Since 0.0.422, when a session ends it writes `session.shutdown` with tokens and requests per model (`modelMetrics`). Veyra records one usage record per run and model, marked as a total (`Bucket` granularity), so prompt-length price tiers cannot be applied exactly.

## Mapping rules

| Rule | Detail |
|---|---|
| Summaries before 1.0.66 | Each covers its own run: every summary counts in full |
| Summaries from 1.0.66 | Each covers the whole session so far: a run is the difference from the previous summary. A summary whose totals went down started over (resumed by an older version) and counts in full. The version comes from `session.start` |
| Keys | `copilot:<session>:<shutdown event id>:<model>`, stable across re-reads |
| Tokens | `inputTokens` includes cached tokens; uncached input = input − cache reads − cache writes. Whether cache writes are included in input is not documented, so cache figures are marked partial |
| Attribution | Gateway `github-copilot`; the vendor comes from the model (`claude-sonnet-4.5` resolves to Anthropic's `claude-sonnet-4-5`); application Copilot CLI |
| Tools | `tool.execution_start` (`toolCallId`, `toolName`) and `tool.execution_complete` (`success`); never arguments or results |
| Project | `context.cwd` of `session.start`, `session.resume` and settled `session.context_changed` |

## Known gaps

- A run that crashed wrote no summary, and its usage is lost (from 1.0.66 the next summary leaves it out too).
- Sessions from before 0.0.422 hold no token data.
- Subagent usage is included in its session's summary and not shown separately.
- Requests made with your own provider key (`providerModelMetrics`) are not read yet.
