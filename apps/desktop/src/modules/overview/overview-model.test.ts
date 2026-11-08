import type { Kpi, Measures, Overview, RankRow } from "@veyra/contracts";
import { describe, expect, it } from "vitest";

import type { ChartTheme } from "@/shared/charts";

import { dailyTotals, hasUsage, modelItems, projectItems, trendInput } from "./overview-model";

const theme: ChartTheme = {
  text: "#000000",
  muted: "#111111",
  subtle: "#777777",
  border: "#333333",
  surface: "#ffffff",
  raised: "#ffffff",
  accent: "#0000ff",
  font: "Inter",
  data: [
    "#d0602a",
    "#0e8f7e",
    "#3567d6",
    "#b5179e",
    "#8f5b00",
    "#5d6b80",
    "#0a84a3",
    "#448a26",
    "#8550e6",
    "#b33f3f",
  ],
  seq: [],
};

const m = (tokens: number, costNanoUsd: number, requests: number): Measures => ({
  tokens,
  costNanoUsd,
  requests,
});
const kpi: Kpi = { value: null, previous: null, change: null, accuracy: null, definition: "" };
const row = (key: string | null, measures: Measures, extra: Partial<RankRow> = {}): RankRow => ({
  key,
  label: key ?? "Other",
  provider: "anthropic",
  measures,
  share: 0,
  groupCount: 1,
  isOther: false,
  ...extra,
});

const overview: Overview = {
  kpis: {
    tokens: kpi,
    costNanoUsd: kpi,
    requests: kpi,
    sessions: kpi,
    activeMs: kpi,
    cacheHitRatio: kpi,
  },
  composition: {
    exact: 1,
    providerReported: 0,
    partial: 0,
    estimated: 0,
    unsupported: 0,
    weakest: "exact",
    pricingCoverage: 1,
  },
  apps: [
    { key: "claude-code", label: "Claude Code" },
    { key: "codex", label: "Codex CLI" },
  ],
  providers: [{ key: "anthropic", label: "Anthropic" }],
  daily: [
    { date: "2026-11-07", values: [m(100, 10, 1), m(50, 40, 2)] },
    { date: "2026-11-08", values: [m(0, 0, 0), m(0, 0, 0)] },
  ],
  models: [
    row("claude-opus-5-5", m(100, 10, 1)),
    row("claude-sonnet-5-5", m(40, 60, 1)),
    row(null, m(10, 5, 1), { isOther: true, groupCount: 3 }),
  ],
  projects: [
    row("1", m(120, 50, 2), { provider: null, label: "/work/a" }),
    row("2", m(30, 20, 1), { provider: null, label: "/work/b" }),
  ],
  calendar: [],
  hours: [],
};

describe("overview model", () => {
  it("detects usage", () => {
    expect(hasUsage(overview)).toBe(true);
    expect(hasUsage({ ...overview, daily: [{ date: "2026-11-08", values: [m(0, 0, 0)] }] })).toBe(
      false,
    );
  });

  it("builds one trend series per application for the measure", () => {
    const { dates, series } = trendInput(overview, "cost", theme);
    expect(dates).toEqual(["2026-11-07", "2026-11-08"]);
    expect(series.map((s) => [s.name, s.values])).toEqual([
      ["Claude Code", [10, 0]],
      ["Codex CLI", [40, 0]],
    ]);
    expect(dailyTotals(overview, "requests")).toEqual([3, 0]);
  });

  it("re-ranks by the selected measure and keeps Other last", () => {
    const byCost = modelItems(overview, "cost", theme);
    expect(byCost.map((i) => i.key)).toEqual(["claude-sonnet-5-5", "claude-opus-5-5", null]);
    expect(byCost.reduce((sum, i) => sum + i.share, 0)).toBeCloseTo(1);
    expect(byCost[2]?.label).toBe("Other (3 models)");
    expect(byCost[0]?.color).toBe("#d0602a");
    expect(byCost[1]?.color).not.toBe("#d0602a");
  });

  it("labels projects with their paths", () => {
    expect(projectItems(overview, "tokens", theme).map((i) => [i.key, i.label])).toEqual([
      ["1", "/work/a"],
      ["2", "/work/b"],
    ]);
  });
});
