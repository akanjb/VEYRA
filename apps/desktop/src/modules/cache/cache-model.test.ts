import type { Cache, Kpi, RatioRow } from "@veyra/contracts";
import { describe, expect, it } from "vitest";

import { TEST_THEME as theme } from "@/shared/charts/test-theme";

import { flowInput, hasCacheUse, ratioItems, ttlInput } from "./cache-model";

const kpi: Kpi = { value: null, previous: null, change: null, accuracy: null, definition: "" };
const ratio = (key: string | null, hitRatio: number | null, extra: Partial<RatioRow> = {}) => ({
  key,
  label: key ?? "Other",
  provider: "anthropic",
  hitRatio,
  inputTokens: 1_000,
  cacheRead: 400,
  isOther: false,
  ...extra,
});

const cache: Cache = {
  kpis: {
    netBenefitNanoUsd: kpi,
    grossSavingNanoUsd: kpi,
    writeOverheadNanoUsd: kpi,
    hitRatio: kpi,
    reuseRatio: kpi,
  },
  accuracy: "exact",
  pricingCoverage: 0.8,
  coverageDefinition: "",
  daily: [
    { date: "2026-11-07", reads: 90, writes: 10 },
    { date: "2026-11-08", reads: 0, writes: 0 },
  ],
  byModel: [
    ratio("claude-opus-5-5", 0.4),
    ratio("unpriced", null),
    ratio(null, 0.1, { isOther: true }),
  ],
  byProject: [],
  writesByModel: [
    {
      key: "claude-opus-5-5",
      label: "Claude Opus 5.5",
      provider: "anthropic",
      short: 5,
      long: 0,
      unspecified: 0,
      reads: 90,
    },
  ],
  inefficient: [],
  inefficientDefinition: "",
};

describe("cache model", () => {
  it("detects cache activity", () => {
    expect(hasCacheUse(cache)).toBe(true);
    expect(hasCacheUse({ ...cache, daily: cache.daily.slice(1) })).toBe(false);
  });

  it("puts reads above the axis and writes below, as magnitudes", () => {
    const { dates, reads, writes } = flowInput(cache, theme);
    expect(dates).toHaveLength(2);
    expect(reads.values).toEqual([90, 0]);
    expect(writes.values).toEqual([10, 0]);
  });

  it("charts only groups with a hit ratio and explains each one", () => {
    const items = ratioItems(cache.byModel, "model", theme);
    expect(items.map((i) => [i.key, i.value])).toEqual([
      ["claude-opus-5-5", 0.4],
      [null, 0.1],
    ]);
    expect(items[0]?.detail).toBe("400 of 1K input tokens");
  });

  it("shows only the lifetimes some model reported, and always the reads", () => {
    const { categories, series } = ttlInput(cache, theme);
    expect(categories).toEqual(["Claude Opus 5.5"]);
    expect(series.map((s) => s.name)).toEqual(["5-minute writes", "Cache reads"]);
  });
});
