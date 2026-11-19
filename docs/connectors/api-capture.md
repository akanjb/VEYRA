# API capture proxy

The capture proxy (ADR-039) records the usage of requests that apps and scripts send to provider APIs through Veyra. It is off by default; turn it on from the Sources page.

## Setting a client up

Replace the provider's base URL with the proxy's. Keys and every other setting stay the same.

| Provider | Base URL to set | Forwarded to |
|---|---|---|
| OpenAI API | `http://127.0.0.1:4320/openai/v1` | `https://api.openai.com/v1` |
| Anthropic API | `http://127.0.0.1:4320/anthropic` | `https://api.anthropic.com` |
| Gemini API | `http://127.0.0.1:4320/gemini` | `https://generativelanguage.googleapis.com` |
| OpenRouter | `http://127.0.0.1:4320/openrouter/v1` | `https://openrouter.ai/api/v1` |

Examples: `OPENAI_BASE_URL` for the OpenAI SDKs, `ANTHROPIC_BASE_URL` for the Anthropic SDKs and Claude Code, and `base_url` or `baseURL` in client constructors.

## What is recorded

Only successful requests to these endpoints are recorded:
- OpenAI wire (OpenAI, OpenRouter): `…/chat/completions`, `…/responses`, `…/completions`, `…/embeddings`.
- Anthropic: `…/messages` (not `count_tokens`).
- Gemini: `…:generateContent`, `…:streamGenerateContent`.

Every other request, such as model lists or file uploads, is relayed but not recorded.

| Wire | Fields read | Canonical mapping |
|---|---|---|
| OpenAI | `id`, `model`, `service_tier`, `usage.prompt_tokens` / `input_tokens`, `*_tokens_details.cached_tokens`, `completion_tokens` / `output_tokens`, `*_tokens_details.reasoning_tokens`; OpenRouter also `usage.cost` | Input includes cached tokens, so uncached = input − cached. Reasoning is part of output. |
| Anthropic | `message.id`, `message.model`, `usage` (merged across `message_start` and `message_delta`), the `request-id` header | The same rules as Claude Code's logs ([claude-code.md](claude-code.md)). |
| Gemini | `responseId`, `modelVersion` (else the model in the path), `usageMetadata` | Uncached = prompt − cached + tool-use prompt; output = candidates + thoughts; reasoning = thoughts. |

Streams are read one event line at a time; non-streamed bodies of up to 16 MiB are read once they end. Gemini streams without `alt=sse` arrive as one JSON array, which is handled.

## Keys and duplicates

| Wire | Natural key |
|---|---|
| OpenAI | `openai:<id>` |
| OpenRouter | `openrouter:<id>` |
| Anthropic | `anthropic:<request-id header>`, the key Claude Code's logs use |
| Gemini | `gemini:<responseId>` |

A response that names no id gets a unique key, so it is never merged with another. Proxied records rank below a completed Claude Code log line for the same request. Tools Veyra reads from their own logs under other keys, such as Codex and Gemini CLI, are counted twice if they are also routed through the proxy.

## Limits

- A streamed OpenAI Chat Completion carries usage only when the request sets `stream_options: {"include_usage": true}`. Without it, the request is counted once, with tokens marked Partial.
- API requests carry no session or working folder; they appear under their provider's API application.
- Request bodies are limited to 64 MiB; at most 32 requests are relayed at once.
