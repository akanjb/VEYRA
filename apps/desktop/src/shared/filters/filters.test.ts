import { describe, expect, it } from "vitest";

import { addDays, daysInclusive, isIsoDate, todayIn } from "./dates";
import {
  clearDimensions,
  filterState,
  rangeDates,
  stepRange,
  toScope,
  toSearch,
  toggleValue,
  validateSearch,
  withDay,
  type FilterState,
} from "./filters";

const TODAY = "2026-11-08";

describe("validateSearch", () => {
  it("drops unknown and invalid values to keep the URL canonical", () => {
    expect(
      validateSearch({
        range: "forever",
        m: "vibes",
        cmp: "maybe",
        provider: "",
        extra: "x",
      }),
    ).toEqual({});
    expect(validateSearch({ range: "custom", from: "2026-11-09", to: "2026-11-01" })).toEqual({});
    expect(validateSearch({ range: "custom", from: "2026-02-30", to: "2026-03-01" })).toEqual({});
  });

  it("keeps valid values, deduplicated", () => {
    expect(
      validateSearch({
        range: "7d",
        cmp: "none",
        m: "cost",
        model: "claude-opus-5-5, claude-opus-5-5,claude-sonnet-5-5",
        project: "12",
      }),
    ).toEqual({
      range: "7d",
      cmp: "none",
      m: "cost",
      model: "claude-opus-5-5,claude-sonnet-5-5",
      project: "12",
    });
  });

  it("omits defaults", () => {
    expect(validateSearch({ range: "30d", m: "tokens" })).toEqual({});
  });
});

describe("filter state", () => {
  it("round-trips through the URL", () => {
    const state: FilterState = {
      range: "custom",
      from: "2026-10-01",
      to: "2026-10-31",
      compare: false,
      measure: "requests",
      dimensions: {
        providers: ["anthropic"],
        models: [],
        apps: [],
        interfaces: ["cli"],
        projects: [],
      },
    };
    expect(filterState(toSearch(state))).toEqual(state);
  });

  it("resolves presets against today", () => {
    const base = filterState({});
    expect(rangeDates(base, TODAY)).toEqual({ start: "2026-10-10", end: TODAY });
    expect(rangeDates({ ...base, range: "today" }, TODAY)).toEqual({ start: TODAY, end: TODAY });
    expect(rangeDates({ ...base, range: "mtd" }, TODAY)).toEqual({
      start: "2026-11-01",
      end: TODAY,
    });
    expect(rangeDates({ ...base, range: "ytd" }, TODAY)).toEqual({
      start: "2026-01-01",
      end: TODAY,
    });
  });

  it("steps whole periods back and forth", () => {
    const week = filterState({ range: "7d" });
    const previous = stepRange(week, -1, TODAY);
    expect(previous).toMatchObject({ range: "custom", from: "2026-10-26", to: "2026-11-01" });
    expect(stepRange(previous, 1, TODAY)).toMatchObject({ from: "2026-11-02", to: TODAY });
  });

  it("builds the scope the commands take", () => {
    const state = filterState({ range: "today", cmp: "none", app: "claude-code" });
    expect(toScope(state, TODAY, "Asia/Kolkata")).toEqual({
      timeZone: "Asia/Kolkata",
      start: TODAY,
      end: TODAY,
      comparison: "none",
      filters: { providers: [], models: [], apps: ["claude-code"], interfaces: [], projects: [] },
    });
  });
});

describe("dates", () => {
  it("does civil arithmetic", () => {
    expect(isIsoDate("2024-02-29")).toBe(true);
    expect(isIsoDate("2026-02-29")).toBe(false);
    expect(addDays("2026-03-01", -1)).toBe("2026-02-28");
    expect(daysInclusive("2026-11-01", "2026-11-07")).toBe(7);
    expect(todayIn("Asia/Kolkata", new Date(Date.UTC(2026, 10, 7, 19, 0)))).toBe("2026-11-08");
  });
});

describe("drill-down", () => {
  it("narrows to a day and toggles filter values", () => {
    const base = filterState({});
    expect(withDay(base, "2026-11-03")).toMatchObject({
      range: "custom",
      from: "2026-11-03",
      to: "2026-11-03",
    });
    const filtered = toggleValue(base, "models", "claude-opus-5-5");
    expect(filtered.dimensions.models).toEqual(["claude-opus-5-5"]);
    expect(toggleValue(filtered, "models", "claude-opus-5-5").dimensions.models).toEqual([]);
    expect(clearDimensions(toggleValue(base, "apps", "claude-code")).dimensions).toEqual(
      base.dimensions,
    );
  });
});
