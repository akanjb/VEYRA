# 0039. Local capture proxy for API usage

- **Status:** Accepted
- **Date:** 2026-11-19

## Context

Usage of the provider APIs, made by the user's own scripts, apps and agent frameworks, leaves no local record that Veyra can read. Every response does report its own usage, though: OpenAI, Anthropic, Gemini and OpenAI-compatible services such as OpenRouter return a documented `usage` block with each request. The coverage assessment ([06-universal-coverage.md](../planning/06-universal-coverage.md), §3.2) ranks a local proxy that the user points a client at as the widest exact coverage after the OpenTelemetry receiver (ADR-038).

Constraints:
- No new network exposure by default.
- Never store or log API keys, prompts or replies (NFR-03, ADR-014).
- No interception of traffic the user did not route to Veyra, and no trusted root certificate.
- Ingestion must not learn any provider's format (the architecture keeps formats in connectors).

## Decision

1. **Opt-in and loopback only.**
   - The user turns the proxy on from the Sources page. The choice is kept in settings and applied at the next launch.
   - It listens on `127.0.0.1:4320`. While it is off, no port is open.
   - A port that cannot be opened is shown on the card, not raised as an error.
2. **Explicit routing, unchanged forwarding.**
   - A client uses `http://127.0.0.1:4320/<provider>` as its base URL. Requests are forwarded over HTTPS to that provider's official origin with the same method, path, query, headers and body.
   - Only headers that describe the local connection are changed. `Accept-Encoding` is set to `identity` so usage can be read on the way.
   - Redirects are not followed.
   - Routes: `openai` (api.openai.com), `anthropic` (api.anthropic.com), `gemini` (generativelanguage.googleapis.com) and `openrouter` (openrouter.ai/api). Adding an OpenAI-compatible provider is one route entry.
3. **Refused requests.**
   - Requests with an `Origin` header, or whose `Host` is not `127.0.0.1` or `localhost` on the proxy's port, are refused. A web page therefore cannot use the proxy as a relay, directly or through DNS rebinding.
   - Request bodies are limited to 64 MiB, and at most 32 connections are relayed at once.
4. **Usage read in passing, never content.**
   - Responses are relayed as they arrive, with chunked framing, so streams are not delayed.
   - For generation requests that succeed, the route's scanner parses each event-stream line, or a non-streamed body of up to 16 MiB, keeps only the response id, model, service tier and usage counts, and drops the rest.
   - Keys pass through in headers and are never stored or logged.
   - Errors are described by kind only, because a URL can carry a key (Gemini's `?key=`).
5. **The connector owns the formats.**
   - `veyra-connectors::capture` defines the routes and the scanners, behind the domain traits `CaptureRoute` and `UsageScan`.
   - `veyra-ingest::Proxy` does the network work and receives the routes by injection, as the receiver receives connectors.
   - Each route records under its own application: OpenAI API, Anthropic API, Gemini API and OpenRouter.
6. **Same keys and pipeline.**
   - Natural keys are the provider's response id (`openai:chatcmpl-…`, `gemini:<responseId>`, `openrouter:gen-…`).
   - For Anthropic the key is the `request-id` header, the key Claude Code's logs use. The proxied record ranks below a completed log line, so pointing Claude Code at the proxy counts each request once.
   - Usage is normalised, priced and written through the same path as the receiver's, and raises the same `data-changed` event.
   - OpenRouter's reported `usage.cost` is kept as the provider-reported cost.
7. **HTTP client.** `reqwest` in blocking mode with the operating system's TLS (`native-tls`: SChannel on Windows, Security.framework on macOS). It needs no extra build toolchain and trusts the same certificates as the system. The client is created on the proxy's own thread, never inside an async runtime.

## Consequences

- Any client whose base URL can be set is covered exactly, with no change to its code beyond that setting. That includes SDKs, agent frameworks and local scripts.
- A streamed OpenAI Chat Completion reports usage only when the client sets `stream_options.include_usage`. Without it the request is still counted, with tokens marked Partial.
- Tools whose own logs Veyra already reads use different keys, so routing them through the proxy as well counts them twice; Codex and Gemini CLI are examples. Claude Code is the exception. The card says so.
- Proxied requests carry no session or project, and are shown under the API application of their provider.
- The proxy adds one local hop. Bodies are not buffered, except non-streamed responses that are read for usage, up to 16 MiB.
- A request in progress when the proxy is turned off is allowed to finish.
- Deleting an API application's data is accepted by the deletion command. The Storage page lists only file sources, so its picker cannot choose one yet.
