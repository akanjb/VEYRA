# Codex fixtures (rollout format as of Codex 0.167)

Synthetic data only, modelled on the rollout structure in the Codex source code (`openai/codex`, `codex-rs/protocol` and `codex-rs/rollout`): field names and shapes, never real content.

`codex-home/` stands for `$CODEX_HOME` (`~/.codex`):

| File | What it exercises |
|---|---|
| `sessions/2026/11/08/rollout-…-0001.jsonl` | A current CLI session: exact `token_usage_record` lines (with `token_count` totals alongside, which must not be counted twice), a model change, tool calls with outcomes, and a malformed last line |
| `sessions/2026/11/08/rollout-…-0002.jsonl` | An older IDE session with running totals only: a repeated total and a rate-limit-only update add nothing, and the counted increases add up to the final total |
| `sessions/2026/11/08/rollout-…-0003.jsonl` | A fork of 0002: the copied totals before its first own turn are not counted, and a total overwritten after compaction starts a new baseline |
| `archived_sessions/rollout-…-0004.jsonl` | A desktop-app subagent thread, archived, with one record copied from its parent that must be skipped |
| `history.jsonl`, `auth.json`, `notes.txt` | Files that are not rollouts and must never be read |

Every place real rollouts hold user content contains a `VEYRA-CANARY` marker. Tests assert that no marker ever reaches the database. Tests also compress a rollout to `.jsonl.zst` to check that a compressed copy gives the same records.
