# 0034. Tools & Agents page and the tool rollup

- **Status:** Accepted
- **Date:** 2026-11-12

## Context

Tokens and cost show what an agent consumed, not what it did. People who work with coding agents ask:
- which tools the agent relies on;
- how much it hands to MCP servers and subagents;
- which tools keep failing and waste requests.

Veyra already stores each tool call as metadata only: its name, its MCP server and whether it failed (PRD NFR-03). Inputs and outputs are never stored. No page showed these calls across sessions.

The page must stay within its budget at ten million events (ADR-030). Reading `tool_calls` joined to their events for a year would not.

## Decision

1. **A tool rollup.** Migration 0009 adds `rollup_15m_tools`. It holds one row per 15-minute bucket, tool and combination of every filterable dimension, with:
   - calls;
   - failed calls;
   - calls whose outcome is known;
   - calls made inside subagents.

   Because every filter dimension is a key, each query reads the rollup alone. No query falls back to raw calls.
2. **Kept exact by the existing rebuild.** The rollup is rebuilt with the other levels for each bucket a write touches. Two cases were added that do not change an event:
   - a new call recorded on an already stored request;
   - an outcome that arrives in a later batch.

   Both add their event's bucket to the rebuild. An outcome that does not change the stored value adds nothing. The migration fills the rollup for existing data. Every level is filled once, after the last migration that adds one, so a partial upgrade can never rebuild into a table that does not yet exist.
3. **Kinds of tool.** `ToolCategory::of` in the domain groups tool names across applications into eight kinds:
   - read and search;
   - edit;
   - shell;
   - web;
   - subagent;
   - planning;
   - MCP;
   - other.

   It is the single definition. The store calls it through the SQL function `veyra_tool_category`, as it does for size bins.
4. **The page (`get_tools`).** It shows:
   - **Headline tiles:** tool calls, calls per request, failure rate, MCP share and the share made by subagents, each compared with the previous period.
   - **Calls per day by kind**, stacked.
   - **The most used tools.**
   - **MCP servers**, with the tools used and their failure rate.
   - **The tools that fail most.**

   The failure rate divides by calls with a known outcome only, so applications that report no outcomes do not lower it.

## Alternatives considered

- **Reading `tool_calls` joined to events on each request.** This is simple, but it scans every call in the period and misses the page budget on large databases.
- **Storing the kind on each call.** A rename of a kind would then need a data migration. Computing the kind in the rebuild keeps one definition, which applies to the next rebuild.
- **Tracking outcomes by changing rollup counts in place.** This is fast, but it breaks the rule that a bucket is always exactly the aggregation of its events (ADR-009).

## Consequences

- Each write that changes calls rebuilds one more table for its buckets.
- A new tool name lands in "Other" until it is added to the kind table; its calls are still counted.
- The page has no token or cost figures per tool. Sources report tokens per request, not per tool call, and splitting them would invent numbers.

## Revisit when

Sources report tokens or durations per tool call, or the OTLP receiver supplies tool spans.
