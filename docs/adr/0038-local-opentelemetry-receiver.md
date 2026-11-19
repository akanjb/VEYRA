# 0038. Local OpenTelemetry receiver

- **Status:** Accepted
- **Date:** 2026-11-19

## Context

Log files are the only source Veyra reads today. Their formats are undocumented and can change with any release. Several tools also offer an official, documented OpenTelemetry export: Claude Code, Codex, Gemini CLI, and Copilot in VS Code. Tools that adopt the OpenTelemetry GenAI conventions would be covered without a new file parser. The roadmap and the coverage assessment ([06-universal-coverage.md](../planning/06-universal-coverage.md)) list an opt-in local receiver as the largest exact-coverage gain per unit of work.

Constraints:
- No new network exposure by default.
- Never store prompts or tool content (NFR-03).
- Reuse the one ingestion path, so pushed and read usage cannot drift apart.
- No heavy dependencies.

## Decision

1. **Off by default; loopback only.**
   - The user turns the receiver on from the Sources page. The choice is kept in settings and applied at the next launch.
   - It listens on `127.0.0.1:4318`, the standard OTLP/HTTP port. While it is off, no port is open.
   - A port that cannot be opened is shown on the card, not raised as an error.
2. **OTLP/HTTP with JSON, log exports only.**
   - It accepts `POST /v1/logs` with `Content-Type: application/json`, uncompressed.
   - Protobuf, gRPC, compression, metrics and traces are refused with a status that names the fix. The documented setup sets the logs protocol to `http/json`.
   - JSON needs no protobuf or gRPC stack. Usage arrives in log events, not in metrics.
3. **Refused requests.**
   - Requests carrying an `Origin` header are refused, as are requests whose `Host` is not `127.0.0.1` or `localhost` on the receiver's port. This blocks web pages and DNS rebinding.
   - Bodies are limited to 8 MiB and headers to 16 KiB, with a 10-second timeout per connection.
   - Connections are answered one at a time, then closed.
   - The HTTP handling is about 150 lines of tested code rather than a server framework.
4. **The connector owns the mapping.**
   - `SourceConnector` gains two optional methods: `telemetry_service`, the `service.name` the tool reports, and `telemetry_usage`, which maps one log record to a `RawUsage`.
   - The receiver decodes an export into records holding scalar attributes only, routes each record to the connector of its service, and drops the rest.
   - Adding a tool is two methods on its connector. The receiver does not change.
5. **Same keys, lower precedence.**
   - Claude Code's documented `api_request` event carries `request_id`, so its natural key `anthropic:<request_id>` is the one the log line uses.
   - The event has no stop reason, so it gets the precedence of an incomplete line. A completed log line, which also knows the project, always supersedes it.
   - The telemetry event never replaces that line, so turning on both counts each request once.
   - `cost_usd` is Claude Code's own estimate, not a charge, and is not kept.
6. **Same pipeline.** Pushed usage is normalised, priced and written through `normalize`, `price_event` and `Store::write_events`, and it raises the same `data-changed` event as a scan.

## Consequences

- Usage pushed by Claude Code appears within seconds; its logs export every 5 seconds by default. Usage that arrives only through telemetry has no project until the log line is read.
- Records are processed in memory and never stored or logged. Prompt or tool content a tool may be configured to send is dropped with the rest of the record.
- Telemetry usage shares the connector, and therefore the application, with the log files. Deleting a connector's data covers both. The source switch on the Sources page governs the log files only; the receiver has its own switch.
- Tools that export only protobuf, or need gRPC, are not covered until protobuf decoding is added behind the same routing.
- Codex, Gemini CLI and Copilot in VS Code each need a validated mapping, with real exports as fixtures, before their connector implements the two methods.
