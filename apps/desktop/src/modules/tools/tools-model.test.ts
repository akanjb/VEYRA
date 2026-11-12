import type { Tools } from "@veyra/contracts";
import { describe, expect, it } from "vitest";

import type { ChartTheme } from "@/shared/charts";

import { failureRate, hasToolCalls, kindSeries, toolItems, toolLabel } from "./tools-model";

const theme = { data: Array.from({ length: 12 }, (_, i) => `#${String(i)}`) } as ChartTheme;

const counts = (calls: number, failed = 0, withOutcome = calls) => ({
  calls,
  failed,
  withOutcome,
});

const tools = {
  kinds: [
    { kind: "shell", counts: counts(3, 1) },
    { kind: "mcp", counts: counts(1) },
  ],
  daily: [
    { date: "2026-11-07", calls: [1, 0] },
    { date: "2026-11-08", calls: [2, 1] },
  ],
  tools: [
    {
      name: "Bash",
      label: "Bash",
      mcpServer: null,
      kind: "shell",
      counts: counts(3, 1),
    },
    {
      name: "mcp__github__get_issue",
      label: "get_issue",
      mcpServer: "github",
      kind: "mcp",
      counts: counts(1),
    },
  ],
} as unknown as Tools;

describe("tools model", () => {
  it("stacks calls per kind and day", () => {
    const { dates, series } = kindSeries(tools, theme);
    expect(dates).toEqual(["2026-11-07", "2026-11-08"]);
    expect(series.map((s) => s.name)).toEqual(["Shell", "MCP tools"]);
    expect(series[1]?.values).toEqual([0, 1]);
    expect(series[0]?.color).not.toBe(series[1]?.color);
  });

  it("ranks tools with their server and share of all calls", () => {
    const items = toolItems(tools, theme);
    expect(items.map((i) => i.label)).toEqual(["Bash", "get_issue (github)"]);
    expect(items[0]?.share).toBe(0.75);
    expect(tools.tools.map(toolLabel)).toEqual(["Bash", "get_issue (github)"]);
  });

  it("gives no failure rate without known outcomes", () => {
    expect(failureRate(counts(4, 1))).toBe(0.25);
    expect(failureRate(counts(4, 0, 0))).toBeNull();
    expect(hasToolCalls(tools)).toBe(true);
    expect(hasToolCalls({ ...tools, kinds: [] })).toBe(false);
  });
});
