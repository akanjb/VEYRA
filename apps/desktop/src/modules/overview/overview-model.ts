/**
 * Maps the Overview response to chart inputs for the selected measure. Pure: no React,
 * no formulas beyond selecting and normalising what the core computed.
 */
import type { Overview } from "@veyra/contracts";

import { categoryColor, type ChartTheme, type RankItem, type TrendSeries } from "@/shared/charts";
import type { Measure } from "@/shared/filters";
import { MEASURE } from "@/shared/format";
import { rankItems } from "@/shared/rankings";

/** Whether the scope has any recorded usage. */
export function hasUsage(overview: Overview): boolean {
  return overview.daily.some((day) => day.values.some((v) => v.requests > 0 || v.tokens > 0));
}

/** The stacked daily trend: one series per application, in the core's order. */
export function trendInput(overview: Overview, measure: Measure, theme: ChartTheme) {
  const pick = MEASURE[measure].pick;
  const dates = overview.daily.map((d) => d.date);
  const series: TrendSeries[] = overview.apps.map((app, i) => ({
    name: app.label,
    color: categoryColor(theme, i),
    values: overview.daily.map((d) =>
      pick(d.values[i] ?? { tokens: 0, costNanoUsd: 0, requests: 0 }),
    ),
  }));
  return { dates, series };
}

/** Daily totals of the measure, for the hero sparkline. */
export function dailyTotals(overview: Overview, measure: Measure): number[] {
  const pick = MEASURE[measure].pick;
  return overview.daily.map((d) => d.values.reduce((sum, v) => sum + pick(v), 0));
}

/** Model mix with provider identity colours (models tinted within their provider). */
export function modelItems(overview: Overview, measure: Measure, theme: ChartTheme): RankItem[] {
  return rankItems(
    overview.models,
    { pick: MEASURE[measure].pick, coloring: "model", noun: "models" },
    theme,
  );
}

/** Project distribution; each project keeps a colour by rank. */
export function projectItems(overview: Overview, measure: Measure, theme: ChartTheme): RankItem[] {
  return rankItems(
    overview.projects,
    { pick: MEASURE[measure].pick, coloring: "category", noun: "projects" },
    theme,
  );
}

export function calendarDays(overview: Overview) {
  return overview.calendar.map((d) => ({ date: d.date, value: d.tokens }));
}

export function hourCells(overview: Overview) {
  return overview.hours.map((h) => ({ weekday: h.weekday, hour: h.hour, value: h.tokens }));
}
