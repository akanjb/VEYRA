/**
 * The Tools & Agents response built from the synthetic dataset, shaped like the core's.
 * Only for development and UI tests: product numbers always come from the Rust engine.
 */
import type {
  McpServerRow,
  Scope,
  ToolCallCounts,
  ToolKind,
  ToolRow,
  Tools,
} from "@veyra/contracts";

import { kpi, localDay, previousRange, select, type Flat } from "./aggregate";
import { daysOf } from "./views";

/** The kinds of the dataset's tools, in the core's display order. */
const KIND_ORDER: readonly ToolKind[] = ["readSearch", "edit", "shell", "mcp"];

function kindOf(name: string): ToolKind {
  if (name.startsWith("mcp__")) return "mcp";
  if (name === "Edit") return "edit";
  if (name === "Bash") return "shell";
  return "readSearch";
}

interface Call {
  name: string;
  mcpServer: string | null;
  isError: boolean | null;
  inSubagent: boolean;
  day: string;
}

function callsOf(selected: readonly Flat[], timeZone: string): Call[] {
  return selected.flatMap(({ session, event }) =>
    event.tools.map((tool) => ({
      ...tool,
      inSubagent: session.parentId !== null,
      day: localDay(event.at, timeZone),
    })),
  );
}

function countsOf(calls: readonly Call[]): ToolCallCounts {
  return {
    calls: calls.length,
    failed: calls.filter((c) => c.isError === true).length,
    withOutcome: calls.filter((c) => c.isError !== null).length,
  };
}

const ratio = (part: number, whole: number) => (whole === 0 ? null : part / whole);

function headline(calls: readonly Call[], requests: number) {
  const counts = countsOf(calls);
  return {
    calls: counts.calls,
    perRequest: ratio(counts.calls, requests),
    failure: ratio(counts.failed, counts.withOutcome),
    mcp: ratio(calls.filter((c) => c.mcpServer !== null).length, counts.calls),
    subagents: ratio(calls.filter((c) => c.inSubagent).length, counts.calls),
  };
}

function byName(calls: readonly Call[]): ToolRow[] {
  const groups = new Map<string, Call[]>();
  for (const call of calls) groups.set(call.name, [...(groups.get(call.name) ?? []), call]);
  return [...groups.entries()].map(([name, group]) => ({
    name,
    label: name.startsWith("mcp__") ? name.split("__").slice(2).join("__") : name,
    mcpServer: group[0]?.mcpServer ?? null,
    kind: kindOf(name),
    counts: countsOf(group),
  }));
}

function servers(rows: readonly ToolRow[]): McpServerRow[] {
  const groups = new Map<string, ToolRow[]>();
  for (const row of rows) {
    if (row.mcpServer !== null)
      groups.set(row.mcpServer, [...(groups.get(row.mcpServer) ?? []), row]);
  }
  return [...groups.entries()].map(([server, group]) => ({
    server,
    tools: group.length,
    counts: {
      calls: group.reduce((n, r) => n + r.counts.calls, 0),
      failed: group.reduce((n, r) => n + r.counts.failed, 0),
      withOutcome: group.reduce((n, r) => n + r.counts.withOutcome, 0),
    },
  }));
}

export function tools(flat: readonly Flat[], scope: Scope): Tools {
  const selected = select(flat, scope, scope.start, scope.end);
  const prev = previousRange(scope.start, scope.end);
  const before = scope.comparison === "previous" ? select(flat, scope, prev.start, prev.end) : null;
  const calls = callsOf(selected, scope.timeZone);
  const now = headline(calls, selected.length);
  const then = before === null ? null : headline(callsOf(before, scope.timeZone), before.length);
  const tile = (value: number | null, previous: number | null | undefined) => ({
    ...kpi(value, previous ?? null, "Fixture: tool calls."),
    accuracy: null,
  });
  const kinds = KIND_ORDER.flatMap((kind) => {
    const ofKind = calls.filter((c) => kindOf(c.name) === kind);
    return ofKind.length === 0 ? [] : [{ kind, counts: countsOf(ofKind) }];
  });
  const rows = byName(calls);
  return {
    kpis: {
      calls: tile(now.calls, then?.calls),
      callsPerRequest: tile(now.perRequest, then?.perRequest),
      failureRate: tile(now.failure, then?.failure),
      mcpShare: tile(now.mcp, then?.mcp),
      subagentShare: tile(now.subagents, then?.subagents),
    },
    kinds,
    daily: daysOf(scope.start, scope.end).map((date) => ({
      date,
      calls: kinds.map(
        (k) => calls.filter((c) => c.day === date && kindOf(c.name) === k.kind).length,
      ),
    })),
    tools: [...rows].sort((a, b) => b.counts.calls - a.counts.calls),
    servers: servers(rows).sort((a, b) => b.counts.calls - a.counts.calls),
    failing: rows
      .filter((r) => r.counts.failed > 0)
      .sort((a, b) => b.counts.failed - a.counts.failed),
    failureDefinition: "Fixture: failed calls over calls with a known outcome.",
  };
}
