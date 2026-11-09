# GitHub Copilot CLI fixtures (session events as of Copilot CLI 1.0)

Synthetic data only, modelled on the session event schema published with the Copilot SDK (`github/copilot-sdk`, `nodejs/src/generated/session-events.ts`) and the CLI changelog: field names and shapes, never real content.

`copilot-home/` stands for `~/.copilot`:

| File | What it exercises |
|---|---|
| `session-state/s-new/events.jsonl` | A 1.0.100 session (cumulative summaries): run one uses one model; run two, after a resume and a model change, adds a second model, so only the difference counts; tool calls with outcomes; a malformed last line |
| `session-state/s-old/events.jsonl` | A 0.0.430 session (one summary per run): both summaries count in full, although the second is larger |
| `session-state/s-crash/events.jsonl` | A session that never ended: no usage |
| `workspace.yaml`, `logs/`, `config.json`, `history-session-state/` | Files that are not event logs and must never be read |

Every place real logs hold user content contains a `VEYRA-CANARY` marker. Tests assert that no marker ever reaches the database.
