# 0005. Canonical token semantics

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

Vendors define token fields differently:
- OpenAI's `input` includes cached tokens; Anthropic's does not.
- Reasoning tokens are part of output for OpenAI.

Reference projects double-counted because of these differences.

## Decision

- **Canonical fields.** Every event stores:
  - `input_uncached`;
  - `cache_read`;
  - `cache_write_5m`, `cache_write_1h` and `cache_write_unspecified`;
  - `output_total`, which **includes** reasoning;
  - `reasoning`, a reported subset that is never added again;
  - `tool_tokens`, where a source reports it separately.
- **One place converts.** Connectors tag the source-native semantics. One central normaliser converts them to the canonical fields.

## Alternatives considered

- **Keeping vendor-native fields:** every metric would need vendor-specific formulas, scattered across the code.

## Consequences

- Totals and costs are comparable across providers.
- Each connector documents its semantics mapping and has golden tests.

## Revisit when

A provider introduces a billable token class that does not fit these fields.
