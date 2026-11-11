import type { BudgetStatus, Cost, Kpi } from "@veyra/contracts";
import { describe, expect, it } from "vitest";

import { TEST_THEME as theme } from "@/shared/charts/test-theme";
import { waterfallRanges } from "@/shared/charts";

import {
  breakdownItems,
  budgetUse,
  compositionRows,
  hasCost,
  outlookInput,
  rateItems,
  valueFraming,
  waterfallBars,
} from "./cost-model";

const kpi = (value: number | null): Kpi => ({
  value,
  previous: null,
  change: null,
  accuracy: null,
  definition: "",
});

const budget = (extra: Partial<BudgetStatus>): BudgetStatus => ({
  id: 1,
  label: "Monthly",
  project: null,
  limitNanoUsd: 100,
  warnPercent: 80,
  monthToDateNanoUsd: 50,
  forecastNanoUsd: 90,
  level: "within",
  forecastLevel: "warning",
  ...extra,
});

function cost(extra: Partial<Cost> = {}): Cost {
  return {
    kpis: {
      apiEquivalentNanoUsd: kpi(500),
      pricingCoverage: kpi(1),
      actualChargeNanoUsd: kpi(0),
      subscriptionSpendNanoUsd: kpi(200),
      costPerRequestNanoUsd: kpi(5),
      costPerMillionOutputNanoUsd: kpi(9),
    },
    accuracy: "exact",
    otherCurrencyEntries: 0,
    daily: [
      {
        date: "2026-11-01",
        apiEquivalentNanoUsd: 0,
        cumulativeNanoUsd: 0,
        previousCumulativeNanoUsd: 3,
      },
      {
        date: "2026-11-02",
        apiEquivalentNanoUsd: 500,
        cumulativeNanoUsd: 500,
        previousCumulativeNanoUsd: 7,
      },
    ],
    month: {
      month: "2026-11",
      asOf: "2026-11-02",
      days: [
        { date: "2026-11-01", cumulativeNanoUsd: 0, projectedNanoUsd: null },
        { date: "2026-11-02", cumulativeNanoUsd: 500, projectedNanoUsd: 500 },
        { date: "2026-11-03", cumulativeNanoUsd: null, projectedNanoUsd: 750 },
      ],
      monthToDateNanoUsd: 500,
      runRateNanoUsd: 250,
      forecastNanoUsd: 750,
      daysLeft: 1,
    },
    budgets: [budget({}), budget({ id: 2, project: { key: "4", label: "veyra" } })],
    breakdowns: [
      {
        dimension: "model",
        rows: [
          {
            key: "small",
            label: "Small",
            provider: "anthropic",
            measures: { tokens: 900, costNanoUsd: 100, requests: 1 },
            share: 0.2,
            groupCount: 1,
            isOther: false,
          },
          {
            key: "large",
            label: "Large",
            provider: "anthropic",
            measures: { tokens: 100, costNanoUsd: 400, requests: 1 },
            share: 0.8,
            groupCount: 1,
            isOther: false,
          },
        ],
      },
    ],
    sessions: [],
    composition: [
      {
        key: null,
        label: "All models",
        provider: null,
        inputNanoUsd: 1,
        cacheReadNanoUsd: 2,
        cacheWriteNanoUsd: 3,
        outputNanoUsd: 4,
      },
    ],
    rates: [
      { key: "large", label: "Large", provider: "anthropic", costPerMillionOutputNanoUsd: 7 },
    ],
    waterfalls: [
      {
        dimension: "model",
        previousNanoUsd: 300,
        currentNanoUsd: 500,
        steps: [
          {
            key: "large",
            label: "Large",
            provider: null,
            changeNanoUsd: 300,
            groupCount: 1,
            isOther: false,
          },
          {
            key: null,
            label: "Other",
            provider: null,
            changeNanoUsd: -100,
            groupCount: 3,
            isOther: true,
          },
        ],
      },
    ],
    definitions: {
      runRate: "",
      forecast: "",
      budget: "",
      waterfall: "",
      subscriptions: "",
      charges: "",
    },
    ...extra,
  };
}

describe("cost model", () => {
  it("knows whether there is any cost", () => {
    expect(hasCost(cost())).toBe(true);
    expect(hasCost(cost({ daily: [] }))).toBe(false);
  });

  it("frames subscriptions beside the estimate without adding them", () => {
    expect(valueFraming(cost())).toEqual({ subscription: 200, apiEquivalent: 500 });
    const none = cost();
    none.kpis.subscriptionSpendNanoUsd = kpi(0);
    expect(valueFraming(none)).toBeNull();
  });

  it("draws only budgets for all usage on the month outlook", () => {
    const { series, thresholds } = outlookInput(cost(), theme);
    expect(thresholds).toEqual([{ label: "Monthly", value: 100, color: theme.danger }]);
    expect(series[0]?.values).toEqual([0, 500, null]);
    expect(series[1]?.values).toEqual([null, 500, 750]);
    expect(series[1]?.dashed).toBe(true);
  });

  it("ranks breakdowns by cost, not by tokens", () => {
    const breakdown = cost().breakdowns[0];
    if (breakdown === undefined) throw new Error("fixture");
    const items = breakdownItems(breakdown, theme);
    expect(items.map((i) => i.label)).toEqual(["Large", "Small"]);
    expect(items[0]?.share).toBeCloseTo(0.8);
  });

  it("keeps each token kind's cost in its own column", () => {
    expect(compositionRows(cost())).toEqual([{ label: "All models", amounts: [1, 2, 3, 4] }]);
  });

  it("turns rates into dots in the core's order", () => {
    expect(rateItems(cost(), theme).map((i) => [i.label, i.value])).toEqual([["Large", 7]]);
  });

  it("builds a waterfall whose steps land on the current total", () => {
    const waterfall = cost().waterfalls[0];
    if (waterfall === undefined) throw new Error("fixture");
    const bars = waterfallBars(waterfall, theme);
    expect(bars.map((b) => b.label)).toEqual([
      "Previous period",
      "Large",
      "Other (3)",
      "This period",
    ]);
    const ranges = waterfallRanges(bars);
    expect(ranges[1]).toEqual({ low: 300, high: 600 });
    expect(ranges[2]).toEqual({ low: 500, high: 600 });
    expect(ranges[3]).toEqual({ low: 0, high: 500 });
  });

  it("caps the budget meter at the limit", () => {
    expect(budgetUse(budget({}))).toBe(0.5);
    expect(budgetUse(budget({ monthToDateNanoUsd: 300 }))).toBe(1);
    expect(budgetUse(budget({ limitNanoUsd: 0 }))).toBe(0);
  });
});
