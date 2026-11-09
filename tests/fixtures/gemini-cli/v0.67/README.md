# Gemini CLI fixtures (chat recording format as of Gemini CLI 0.67)

Synthetic data only, modelled on the chat recording service in the Gemini CLI source code (`google-gemini/gemini-cli`, `packages/core/src/services/chatRecordingService.ts`): field names and shapes, never real content.

`gemini-tmp/` stands for `~/.gemini/tmp`, holding one project folder, `shop`:

| File | What it exercises |
|---|---|
| `shop/.project_root` | The project folder of every session in `shop` |
| `shop/chats/session-…-abcdef12.jsonl` | A current session: a message written three times as tokens and tool status arrive (counted once), a response without a model (takes the nearest model), a rewind (its tokens still count), a patch, a tool error and a malformed last line |
| `shop/chats/session-…-11112222.json` | A session from before v0.39: one document holding every message |
| `shop/chats/session-…-11112222.jsonl` | The same session resumed by a newer version: its messages copied (not counted twice) plus one new response |
| `shop/chats/abcdef12-…/agent-7.jsonl` | A subagent session of `abcdef12` |
| `shop/logs.json`, `shop/checkpoint-saved.json`, `*.tmp-123`, `*.unreadable-…` | Files that are not recordings and must never be read |

Every place real recordings hold user content contains a `VEYRA-CANARY` marker. Tests assert that no marker ever reaches the database.
