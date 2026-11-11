import type { Activity, Kpi } from "@veyra/contracts";
import { describe, expect, it } from "vitest";

import { TEST_THEME as theme } from "@/shared/charts/test-theme";

import { busiestWeekdays, dailyInput, hasActivity, lengthBins, peakHours } from "./activity-model";

const kpi: Kpi = { value: null, previous: null, change: null, accuracy: null, definition: "" };

const activity: Activity = {
  kpis: { activeMs: kpi, wallMs: kpi, sessions: kpi, activeDays: kpi },
  calendar: [],
  hours: [
    { weekday: 0, hour: 9, tokens: 100 },
    { weekday: 2, hour: 9, tokens: 50 },
    { weekday: 2, hour: 14, tokens: 300 },
    { weekday: 6, hour: 23, tokens: 0 },
  ],
  daily: [
    { date: "2026-11-07", sessions: 2, activeMs: 600_000, wallMs: 1_800_000 },
    { date: "2026-11-08", sessions: 0, activeMs: 0, wallMs: 0 },
  ],
  sessionLengths: [
    { lowerMs: 0, upperMs: 60_000, byActive: 1, byWall: 0 },
    { lowerMs: 60_000, upperMs: null, byActive: 1, byWall: 2 },
  ],
  longest: [],
  idleThresholdMs: 300_000,
  activeDefinition: "",
  wallDefinition: "",
};

describe("activity model", () => {
  it("detects activity", () => {
    expect(hasActivity(activity)).toBe(true);
    expect(hasActivity({ ...activity, daily: activity.daily.slice(1) })).toBe(false);
  });

  it("pairs active and wall time per day", () => {
    const { categories, series } = dailyInput(activity, theme);
    expect(categories).toEqual(["2026-11-07", "2026-11-08"]);
    expect(series.map((s) => [s.name, s.values])).toEqual([
      ["Active time", [600_000, 0]],
      ["Wall time", [1_800_000, 0]],
    ]);
  });

  it("switches the length distribution between active and wall time", () => {
    expect(lengthBins(activity, "active").map((b) => b.count)).toEqual([1, 1]);
    expect(lengthBins(activity, "wall").map((b) => [b.lower, b.upper, b.count])).toEqual([
      [0, 60_000, 0],
      [60_000, null, 2],
    ]);
  });

  it("ranks hours and weekdays by tokens, leaving out empty ones", () => {
    expect(peakHours(activity, theme).map((i) => [i.label, i.value])).toEqual([
      ["14:00–15:00", 300],
      ["09:00–10:00", 150],
    ]);
    const days = busiestWeekdays(activity, theme);
    expect(days.map((d) => d.label)).toEqual(["Wed", "Mon"]);
    expect(days.reduce((n, d) => n + d.share, 0)).toBeCloseTo(1);
  });
});
