/** A small Overview response shared by unit tests (not part of the app bundle). */
import type { Kpi, Measures, Overview, RankRow } from "@veyra/contracts";

export const m = (tokens: number, costNanoUsd: number, requests: number): Measures => ({
  tokens,
  costNanoUsd,
  requests,
});
export const kpi: Kpi = {
  value: null,
  previous: null,
  change: null,
  accuracy: null,
  definition: "",
};
export const row = (
  key: string | null,
  measures: Measures,
  extra: Partial<RankRow> = {},
): RankRow => ({
  key,
  label: key ?? "Other",
  provider: "anthropic",
  measures,
  share: 0,
  groupCount: 1,
  isOther: false,
  ...extra,
});

export const SAMPLE_OVERVIEW: Overview = {
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
  highlights: [],
};
