# 0023. Incremental, consent-gated ingestion

- **Status:** Accepted, extended by [0025](0025-reading-modes-and-capabilities.md) (whole-file and database reading)
- **Date:** 2026-11-08

## Context

M2 turns local tool logs into stored usage, continuously and safely. The Claude Code validation spike (see [`docs/connectors/claude-code.md`](../connectors/claude-code.md)) showed:
- Logs are append-only JSON Lines, often hundreds of megabytes per session, mostly tool output Veyra must never read as content.
- One API request appears on up to 40 lines, and sometimes in several files of one session.
- The tool keeps writing, renaming and deleting these files while Veyra reads them.
- Users must be able to see what Veyra reads and decide whether it may.

## Decision

1. **Consent first.** Discovery only checks that a documented root directory exists and registers it as a source, **disabled**. Nothing under a root is opened until the user enables the source, and the time of consent is recorded. Disabling a source stops reading and watching it at once. Stored history is kept.

2. **Pure connectors, one pipeline.** A connector (`SourceConnector`) is a pure parser: it declares its roots, a strict allow-list of source files and a parse function from complete lines to raw records. It never touches the filesystem, database or network. The ingest crate owns everything else:
   - scoped reads;
   - checkpoints;
   - normalisation through the registry;
   - pricing;
   - persistence.

3. **Scoped, read-only file access.** `ScopedFs` reaches only paths below the source root:
   - plain path components only;
   - no symbolic links;
   - a name deny-list for credential and key files, even inside a root.

   Files are opened read-only with full sharing on Windows.

4. **Checkpoints committed with the data.** Each file has a checkpoint:
   - the offset after its last complete line;
   - its size and modification time;
   - a fingerprint of its first 4 KiB;
   - the parser version.

   Each step's events, tool outcomes and the new checkpoint are committed in **one transaction**, so a crash or cancellation never loses or double-counts data. A file is re-read from the start when it shrank (truncated), its first bytes changed (replaced) or the parser changed. Re-reading is safe because writes are idempotent. A trailing partial line is left for the next scan.

5. **Order-independent deduplication.** Every observation has a precedence (completed copies first, then the most tokens). A lower precedence never replaces a higher one; on a tie the earlier timestamp wins. Results do not depend on the order in which files or chunks are read.

6. **Watcher as a hint.** One background thread scans:
   - at startup;
   - about two seconds after file-change notifications (debounced; at most one scan per window while files keep changing);
   - every 15 minutes;
   - on request.

   Notifications only make a scan happen sooner. Correctness rests on comparing files with their checkpoints, so a missed notification costs latency, never data. Between scans the thread blocks, using no CPU.

7. **Overlapped reading and writing.** Within a file, a helper thread reads, parses, normalises and prices the next 8 MiB chunk while the worker commits the previous one, in strict file order. Measured on the build machine, this lifted end-to-end ingestion from about 35,000 to about 50,000–60,000 events per second.

8. **Tool calls as metadata only.** Tool-call ids, names (with MCP server and tool split out) and error outcomes are stored. Arguments and outputs never are.

9. **Ports in the domain.** `SourceRepository` (implemented by the store) and `IngestControl` (implemented by the ingest worker) are domain traits. The application layer depends only on the domain, which keeps SQLite and the file watcher out of the Tauri build script, since that compiles the command catalogue.

## Alternatives considered

- **Watcher as the source of truth.** Rejected: notifications are lost under load, on network drives and across sleep. Each platform also coalesces them differently.
- **Re-parse whole files on change.** Rejected: active sessions grow to hundreds of megabytes; re-parsing them every few seconds would waste CPU and battery.
- **Store the latest copy instead of the most complete one.** Rejected: the spike found superseded copies with inconsistent values arriving after the final copy, so "last write wins" would make results depend on read order.
- **An async runtime for ingestion.** Rejected for now: the work is CPU- and disk-bound, and two plain threads (worker plus per-file reader) are simpler to reason about and cancel.

## Consequences

- Sources must be enabled before any number appears. The M3 Sources page makes this one click.
- Stored events survive deletion of their source files. Files that disappear are marked missing.
- A connector for a new tool is one module: roots, allow-list, parse, fixtures and a documentation page.
- Process-priority lowering and retry backoff with jitter were not needed to meet the budgets and are deferred. Failures are recorded per source, and the periodic scan retries.

## Revisit when

- A source is not append-only (for example, a live SQLite database: see ADR-015), or needs network access.
- Watching thousands of roots becomes necessary, which would make one recursive watcher per root too costly.
