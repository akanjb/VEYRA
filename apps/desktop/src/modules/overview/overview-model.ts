/**
 * Maps the Overview response to chart inputs for the selected measure. Pure: no React,
 * no formulas beyond selecting and normalising what the core computed.
 */
import type { Overview, RankRow } from "@veyra/contracts";

import {
  categoryColor,
  modelColors,
  otherColor,
  type ChartTheme,
  type RankItem,
  type TrendSeries,
} from "@/shared/charts";
import type { Measure } from "@/shared/filters";
import { MEASURE } from "@/shared/format";

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

/** Shares of the selected measure (the core's shares are of tokens). */
function shares(rows: readonly RankRow[], measure: Measure): number[] {
  const pick = MEASURE[measure].pick;
  const total = rows.reduce((sum, r) => sum + pick(r.measures), 0);
  return rows.map((r) => (total === 0 ? 0 : pick(r.measures) / total));
}

function sortedByMeasure(rows: readonly RankRow[], measure: Measure): RankRow[] {
  const pick = MEASURE[measure].pick;
  const named = rows.filter((r) => !r.isOther).sort((a, b) => pick(b.measures) - pick(a.measures));
  return [...named, ...rows.filter((r) => r.isOther)];
}

/** Model mix with provider identity colours (models tinted within their provider). */
export function modelItems(overview: Overview, measure: Measure, theme: ChartTheme): RankItem[] {
  const rows = sortedByMeasure(overview.models, measure);
  const colors = modelColors(
    theme,
    rows.filter((r) => r.key !== null).map((r) => ({ id: r.key ?? "", provider: r.provider })),
  );
  const share = shares(rows, measure);
  return rows.map((r, i) => ({
    key: r.isOther ? null : r.key,
    label: r.isOther ? `Other (${String(r.groupCount)} models)` : r.label,
    value: MEASURE[measure].pick(r.measures),
    share: share[i] ?? 0,
    color: r.key === null ? otherColor(theme) : (colors.get(r.key) ?? otherColor(theme)),
  }));
}

/** Project distribution; each project keeps a colour by rank. */
export function projectItems(overview: Overview, measure: Measure, theme: ChartTheme): RankItem[] {
  const rows = sortedByMeasure(overview.projects, measure);
  const share = shares(rows, measure);
  return rows.map((r, i) => ({
    key: r.isOther ? null : r.key,
    label: r.isOther ? `Other (${String(r.groupCount)} projects)` : r.label,
    value: MEASURE[measure].pick(r.measures),
    share: share[i] ?? 0,
    color: r.isOther || r.key === null ? otherColor(theme) : categoryColor(theme, i),
  }));
}

export function calendarDays(overview: Overview) {
  return overview.calendar.map((d) => ({ date: d.date, value: d.tokens }));
}

export function hourCells(overview: Overview) {
  return overview.hours.map((h) => ({ weekday: h.weekday, hour: h.hour, value: h.tokens }));
}
