# Claude Code fixtures (format v2.1)

Synthetic data only. Modelled on the structure of Claude Code 2.1.x session logs observed during the M2 validation spike: field names and shapes, never real content.

- `projects/C--work-demo/<session>.jsonl`: a main session with streamed duplicate lines, a superseded final copy carrying anomalous values, a `<synthetic>` line, a malformed line, tool calls (including MCP), tool results and an attachment.
- `projects/C--work-demo/<session>/subagents/agent-a1.jsonl`: a subagent (sidechain) of that session.
- `tool-results/`, `memory/`: files that are **not** usage logs and must never be read.

Every place real logs hold user content contains a `VEYRA-CANARY-…` marker. Tests assert that no marker ever reaches the database.
