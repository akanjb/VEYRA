import type { Kpi, Overview, RankRow } from "@veyra/contracts";
import { describe, expect, it } from "vitest";

import { filterState, toggleValue } from "@/shared/filters";

import { activeFilters, filterOptions } from "./filter-options";

const kpi: Kpi = { value: null, previous: null, change: null, accuracy: null, definition: "" };
const measures = { tokens: 1, costNanoUsd: 1, requests: 1 };
const row = (
  key: string | null,
  label: string,
  provider: string | null,
  isOther = false,
): RankRow => ({
  key,
  label,
  provider,
  measures,
  share: 0,
  groupCount: 1,
  isOther,
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
  apps: [{ key: "claude-code", label: "Claude Code" }],
  providers: [
    { key: "openai", label: "OpenAI" },
    { key: "anthropic", label: "Anthropic" },
  ],
  daily: [],
  models: [
    row("claude-sonnet-5-5", "Claude Sonnet 5.5", "anthropic"),
    row("gpt-6", "GPT-6", "openai"),
    row(null, "Other", null, true),
  ],
  projects: [row("7", "c:/code/veyra", null)],
  calendar: [],
  hours: [],
  highlights: [],
};

describe("filter options", () => {
  it("offers only values present in the data, labelled", () => {
    const options = filterOptions(overview);
    expect(options.providers).toEqual([
      { value: "anthropic", label: "Anthropic" },
      { value: "openai", label: "OpenAI" },
    ]);
    expect(options.apps).toEqual([{ value: "claude-code", label: "Claude Code" }]);
    expect(options.models.map((o) => o.label)).toEqual(["Claude Sonnet 5.5", "GPT-6"]);
    expect(options.projects).toEqual([{ value: "7", label: "veyra" }]);
    expect(filterOptions(undefined).models).toEqual([]);
  });

  it("labels active filters and falls back to raw values", () => {
    const state = toggleValue(toggleValue(filterState({}), "models", "gpt-6"), "projects", "99");
    expect(activeFilters(state, filterOptions(overview))).toEqual([
      { dimension: "models", value: "gpt-6", label: "GPT-6" },
      { dimension: "projects", value: "99", label: "99" },
    ]);
  });
});
