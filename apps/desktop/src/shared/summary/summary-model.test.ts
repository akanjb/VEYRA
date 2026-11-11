import { describe, expect, it } from "vitest";

import { TEST_THEME as theme } from "../charts/test-theme";
import { SAMPLE_OVERVIEW, m } from "../testing/sample-overview";

import { dailyTotals, hasUsage, modelItems, trendInput } from "./summary-model";

const overview = SAMPLE_OVERVIEW;

describe("usage summary model", () => {
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
});
