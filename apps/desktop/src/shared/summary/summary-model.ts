/**
 * Maps a usage summary (the Overview, or one project's detail) to chart inputs for the
 * selected measure. Pure: no React, no formulas beyond selecting and normalising what the
 * core computed.
 */
import type { Overview } from "@veyra/contracts";

import { categoryColor, type ChartTheme, type RankItem, type TrendSeries } from "../charts";
import type { Measure } from "../filters";
import { MEASURE } from "../format";
import { rankItems } from "../rankings";

/** What the Overview and a project detail have in common. */
export type UsageSummary = Pick<Overview, "kpis" | "composition" | "apps" | "daily" | "models">;
/** Whether the scope has any recorded usage. */
export function hasUsage(summary: UsageSummary): boolean {
  return summary.daily.some((day) => day.values.some((v) => v.requests > 0 || v.tokens > 0));
}

/** The stacked daily trend: one series per application, in the core's order. */
export function trendInput(summary: UsageSummary, measure: Measure, theme: ChartTheme) {
  const pick = MEASURE[measure].pick;
  const dates = summary.daily.map((d) => d.date);
  const series: TrendSeries[] = summary.apps.map((app, i) => ({
    name: app.label,
    color: categoryColor(theme, i),
    values: summary.daily.map((d) =>
      pick(d.values[i] ?? { tokens: 0, costNanoUsd: 0, requests: 0 }),
    ),
  }));
  return { dates, series };
}

/** Daily totals of the measure, for the hero sparkline. */
export function dailyTotals(summary: UsageSummary, measure: Measure): number[] {
  const pick = MEASURE[measure].pick;
  return summary.daily.map((d) => d.values.reduce((sum, v) => sum + pick(v), 0));
}

/** Model mix with provider identity colours (models tinted within their provider). */
export function modelItems(summary: UsageSummary, measure: Measure, theme: ChartTheme): RankItem[] {
  return rankItems(
    summary.models,
    { pick: MEASURE[measure].pick, coloring: "model", noun: "models" },
    theme,
  );
}
