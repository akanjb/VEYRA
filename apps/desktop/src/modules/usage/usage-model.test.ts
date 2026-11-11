import type { Breakdown, Kpi, RankRow, Usage } from "@veyra/contracts";
import { describe, expect, it } from "vitest";

import { TEST_THEME as theme } from "@/shared/charts/test-theme";

import {
  BREAKDOWN,
  bandPoints,
  breakdownItems,
  breakdownOf,
  compositionInput,
  hasUsage,
  histogramBins,
} from "./usage-model";

const kpi: Kpi = { value: null, previous: null, change: null, accuracy: null, definition: "" };
const row = (key: string | null, tokens: number, extra: Partial<RankRow> = {}): RankRow => ({
  key,
  label: key ?? "Other",
  provider: null,
  measures: { tokens, costNanoUsd: 0, requests: 1 },
  share: 0,
  groupCount: 1,
  isOther: false,
  ...extra,
});

const projects: Breakdown = {
  dimension: "project",
  rows: [row("1", 60), row("2", 30), row(null, 10, { isOther: true, groupCount: 4 })],
  cumulativeShare: [0.6, 0.9, 1],
  concentration: 0.46,
};

const usage: Usage = {
  kpis: {
    tokens: kpi,
    inputTokens: kpi,
    outputTokens: kpi,
    reasoningTokens: kpi,
    requests: kpi,
    tokensPerRequest: kpi,
  },
  accuracy: "exact",
  composition: [
    {
      date: "2026-11-07",
      inputUncached: 5,
      cacheRead: 50,
      cacheWrite: 10,
      output: 20,
      reasoning: 8,
    },
    {
      date: "2026-11-08",
      inputUncached: 0,
      cacheRead: 0,
      cacheWrite: 0,
      output: 0,
      reasoning: null,
    },
  ],
  reasoningReported: true,
  breakdowns: [projects],
  perRequest: [{ date: "2026-11-07", count: 3, mean: 28.3, p50: 20, p90: 60 }],
  perSession: [],
  requestSizes: [
    { lower: 1024, upper: 1218, requests: 2 },
    { lower: 4294967296, upper: null, requests: 1 },
  ],
  requestDefinition: "",
  sessionDefinition: "",
  concentrationDefinition: "",
};

describe("usage model", () => {
  it("detects usage from the token kinds", () => {
    expect(hasUsage(usage)).toBe(true);
    expect(hasUsage({ ...usage, composition: usage.composition.slice(1) })).toBe(false);
  });

  it("stacks the four token kinds and overlays reasoning only where reported", () => {
    const { dates, series, overlay } = compositionInput(usage, theme);
    expect(dates).toEqual(["2026-11-07", "2026-11-08"]);
    expect(series.map((s) => s.values[0])).toEqual([5, 50, 10, 20]);
    expect(overlay?.values).toEqual([8, null]);
    expect(compositionInput({ ...usage, reasoningReported: false }, theme).overlay).toBeUndefined();
  });

  it("keeps the core's order so the Pareto line stays aligned, Other last", () => {
    const breakdown = breakdownOf(usage, "project");
    expect(breakdown).toBe(projects);
    const items = breakdownItems(projects, theme);
    expect(items.map((i) => i.key)).toEqual(["1", "2", null]);
    expect(items[2]?.label).toBe("Other (4 projects)");
    expect(items.reduce((sum, i) => sum + i.share, 0)).toBeCloseTo(1);
    expect(breakdownOf(usage, "model")).toBeUndefined();
  });

  it("filters only the dimensions the query bar knows", () => {
    expect(BREAKDOWN.model.filter).toBe("models");
    expect(BREAKDOWN.device.filter).toBeNull();
  });

  it("passes sizes through unchanged", () => {
    expect(bandPoints(usage.perRequest)[0]).toEqual(usage.perRequest[0]);
    expect(histogramBins(usage).map((b) => [b.lower, b.upper, b.count])).toEqual([
      [1024, 1218, 2],
      [4294967296, null, 1],
    ]);
  });
});
