# 0025. Reading modes, side files and capability matrices for connectors

- **Status:** Accepted
- **Date:** 2026-11-10

## Context

ADR-023 built ingestion around one storage shape: append-only JSON Lines read incrementally from a byte offset. The next four tools store usage differently:

- **Codex** writes rollouts, sometimes zstd-compressed. Older versions record usage as running totals, so a line's meaning depends on earlier lines.
- **Gemini CLI** writes a message again in full each time it changes. Versions before 0.39 rewrite one JSON document. The project folder sits in a separate small file.
- **OpenCode** keeps everything in a live SQLite database that it writes continuously.
- **Copilot CLI** writes no per-request usage at all, only per-session totals whose meaning changed in version 1.0.66.

Each tool's format was established from its source code or published schema, because these tools had little or no local data on the development machine.

## Decision

1. **Connectors declare how each file is read** (`Reading`), replacing the per-connector format:
   - `AppendLines`: incremental, as in ADR-023 (Claude Code).
   - `WholeFile(Compression)`: re-read in full when the file's size or modification time changes. Used for formats whose records depend on earlier lines or are rewritten. Zstd archives are decoded in memory with the pure-Rust `ruzstd` decoder. Files are capped at 256 MiB after decompression, so a corrupt or hostile archive cannot exhaust memory.
   - `Database(queries)`: the read-only snapshot of ADR-015. The pipeline copies the database in one backup step into a private temporary file, runs the connector's queries and deletes the copy. Each result row is passed to the parser as one JSON line, so connectors remain pure parsers. Queries name the tables they need and are skipped when a table is missing, since each application version has a different set. The database is re-read when its file, or a non-empty write-ahead log, changes.

2. **Side files.** A connector may name small files it needs as context, such as Gemini's `.project_root`. The pipeline reads each through the same root-scoped, deny-listed access, at most 4 KiB.

3. **Gateways.** Usage records may name the gateway that routed a request (GitHub Copilot, OpenRouter, OpenCode Zen). The vendor then comes from the model, so Claude used through Copilot is Anthropic usage, routed by GitHub Copilot, in the Copilot CLI application.

4. **Gateway model names.** The registry resolves a model name exactly first. It then tries the name without a `vendor/` prefix and with dots read as hyphens, so `claude-sonnet-4.5` and `anthropic/claude-sonnet-4.5` resolve to `claude-sonnet-4-5`. Names with dots of their own, such as `gpt-5.5`, still match exactly.

5. **Capability matrices.** Each connector states what it reports (tokens, cache, reasoning, tools, sessions, projects) as full, partial or not reported. Every partial item carries a plain-language reason. The Sources page shows the matrix, and lists the supported tools not found on the device, both from the connectors of the build.

6. **Copilot CLI is supported at session granularity.** Its usage is recorded per session run and model with `Bucket` granularity and provider-reported accuracy, following the summary rule that changed in 1.0.66. Its documentation states that a crashed run is lost.

## Alternatives considered

- **Carrying parser state across appended chunks** (for Codex totals). Rejected for now: reading changed files in full is simpler and always correct, and rollouts are small enough. Revisit if large active rollouts make rescans costly.
- **Reading OpenCode's live database directly.** Rejected: ADR-015 requires a snapshot so that Veyra never holds a transaction on another application's database.
- **Copying the database into memory** instead of a temporary file. Rejected: memory would grow with OpenCode's entire event log and attachments.
- **Deferring Copilot CLI** until it records requests. Rejected: its session totals are exact as Copilot counts them, and the limits can be stated plainly.
- **The `zstd` crate** (bindings to the C library). Rejected in favour of `ruzstd`, which needs no C toolchain on any platform.

## Consequences

- An active whole-file source is re-read on each change. Records are idempotent, so this costs reading time but never correctness.
- Copilot CLI usage appears only after a session ends, and prompt-length price tiers cannot be applied to it exactly.
- Gemini CLI totals can be below Google's bill, because background requests are not recorded. Its capability matrix says so.
- The snapshot path is covered by a test in which the owning application keeps writing, with a busy timeout of 100 ms (a fiftieth of OpenCode's 5 seconds), while Veyra reads. A timeout of zero cannot be guaranteed: while a commit is in progress, any SQLite reader, including the application's own, may hold the WAL write lock for an instant to re-read the WAL index.

## Revisit when

- A tool documents or changes its format: update its connector and bump its parser version.
- Copilot CLI starts persisting per-request usage.
- Rescans of large whole files show up in the ingest budget.
