import type { ToolCallCounts, ToolKind, ToolRow, Tools } from "@veyra/contracts";

import { categoryColor, type ChartTheme, type RankItem, type TrendSeries } from "@/shared/charts";

/** How each kind of tool call is named, in the core's display order. */
export const KIND_LABEL: Readonly<Record<ToolKind, string>> = {
  readSearch: "Read & search",
  edit: "Edit files",
  shell: "Shell",
  web: "Web",
  subagent: "Subagents",
  planning: "Planning",
  mcp: "MCP tools",
  other: "Other",
};

const KINDS = Object.keys(KIND_LABEL) as ToolKind[];

/** A kind's colour, stable across pages and periods. */
export function kindColor(kind: ToolKind, theme: ChartTheme): string {
  return categoryColor(theme, KINDS.indexOf(kind));
}

/** Whether the period holds any tool call. */
export function hasToolCalls(tools: Tools): boolean {
  return tools.kinds.length > 0;
}

/** Failed calls over calls with a known outcome; `null` when no outcome is known. */
export function failureRate(counts: ToolCallCounts): number | null {
  return counts.withOutcome === 0 ? null : counts.failed / counts.withOutcome;
}

/** Calls per local day, one stacked series per kind. */
export function kindSeries(tools: Tools, theme: ChartTheme) {
  const series: TrendSeries[] = tools.kinds.map((row, i) => ({
    name: KIND_LABEL[row.kind],
    color: kindColor(row.kind, theme),
    values: tools.daily.map((d) => d.calls[i] ?? 0),
  }));
  return { dates: tools.daily.map((d) => d.date), series };
}

/** What a tool is called on the page: an MCP tool with its server. */
export function toolLabel(row: ToolRow): string {
  return row.mcpServer === null ? row.label : `${row.label} (${row.mcpServer})`;
}

/** The most called tools as ranking bars, coloured by kind. */
export function toolItems(tools: Tools, theme: ChartTheme): RankItem[] {
  const total = tools.kinds.reduce((sum, k) => sum + k.counts.calls, 0);
  return tools.tools.map((row) => ({
    key: null,
    label: toolLabel(row),
    value: row.counts.calls,
    share: total === 0 ? 0 : row.counts.calls / total,
    color: kindColor(row.kind, theme),
    detail: KIND_LABEL[row.kind],
  }));
}
