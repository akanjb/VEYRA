import type { ModelRow, Models } from "@veyra/contracts";
import { describe, expect, it } from "vitest";

import { TEST_THEME as theme } from "@/shared/charts/test-theme";

import {
  COMPARE_MAX,
  comparedItems,
  hasModels,
  heatMatrix,
  rowId,
  shareInput,
  toggleCompared,
} from "./models-model";

const model = (key: string | null, tokens: number, extra: Partial<ModelRow> = {}): ModelRow => ({
  key,
  label: key ?? "Unknown model",
  provider: "anthropic",
  providerLabel: "Anthropic",
  measures: { tokens, costNanoUsd: 10, requests: 1 },
  costPerMillionOutputNanoUsd: 5,
  cacheHitRatio: null,
  pricingCoverage: 1,
  firstDay: "2026-11-07",
  lastDay: "2026-11-08",
  ...extra,
});

const models: Models = {
  accuracy: "exact",
  dates: ["2026-11-07", "2026-11-08"],
  providers: [
    { key: "anthropic", label: "Anthropic" },
    { key: "openai", label: "OpenAI" },
  ],
  providerShare: [
    { date: "2026-11-07", shares: [0.75, 0.25] },
    { date: "2026-11-08", shares: [0, 0] },
  ],
  leaderboard: [model("claude-opus-5-5", 100), model(null, 20)],
  apps: [{ key: "claude-code", label: "Claude Code" }],
  providerByApp: {
    rows: [{ key: "anthropic", label: "Anthropic" }],
    columns: [{ key: "claude-code", label: "Claude Code" }],
    values: [[120]],
  },
  modelByApp: { rows: [], columns: [], values: [] },
  timeline: [],
  rateDefinition: "",
};

describe("models model", () => {
  it("detects model usage", () => {
    expect(hasModels(models)).toBe(true);
    expect(hasModels({ ...models, leaderboard: [] })).toBe(false);
  });

  it("draws each provider's daily share in its identity hue", () => {
    const { dates, series } = shareInput(models, theme);
    expect(dates).toEqual(models.dates);
    expect(series.map((s) => [s.name, s.values])).toEqual([
      ["Anthropic", [0.75, 0]],
      ["OpenAI", [0.25, 0]],
    ]);
    expect(series[0]?.color).toBe(theme.data[0]);
  });

  it("labels matrices for the heat table", () => {
    expect(heatMatrix(models.providerByApp)).toEqual({
      rows: ["Anthropic"],
      columns: ["Claude Code"],
      values: [[120]],
    });
  });

  it("compares up to the maximum, in the order picked, skipping missing values", () => {
    let picked: string[] = [];
    for (const id of ["a", "b", "c", "d", "e"]) picked = toggleCompared(picked, id);
    expect(picked).toHaveLength(COMPARE_MAX);
    expect(toggleCompared(picked, "b")).toEqual(["a", "c", "d"]);
    const rows = [models.leaderboard[1], models.leaderboard[0]].filter((r) => r !== undefined);
    const items = comparedItems(rows, (r) => r.cacheHitRatio, theme);
    expect(items).toEqual([]);
    expect(comparedItems(rows, (r) => r.measures.tokens, theme).map((i) => i.value)).toEqual([
      20, 100,
    ]);
    expect(rowId(models.leaderboard[1] ?? model(null, 0))).toBe("anthropic/Unknown model");
  });
});
