/**
 * Maps the Activity response to chart inputs. Pure: no React, and no formulas beyond
 * selecting and arranging what the core computed.
 */
import type { Activity } from "@veyra/contracts";

import {
  WEEKDAYS,
  type ChartTheme,
  type GroupSeries,
  type HistogramBin,
  type RankItem,
} from "@/shared/charts";

/** Which session length the distribution shows. */
export type LengthMeasure = "active" | "wall";

/** Whether the period has any activity. */
export function hasActivity(activity: Activity): boolean {
  return activity.daily.some((d) => d.sessions > 0);
}

/** Active and wall time side by side per day. */
export function dailyInput(activity: Activity, theme: ChartTheme) {
  const series: GroupSeries[] = [
    {
      name: "Active time",
      color: theme.data[2] ?? theme.accent,
      values: activity.daily.map((d) => d.activeMs),
    },
    {
      name: "Wall time",
      color: theme.data[5] ?? theme.accent,
      values: activity.daily.map((d) => d.wallMs),
    },
  ];
  return { categories: activity.daily.map((d) => d.date), series };
}

/** Sessions per length range, by the chosen measure. */
export function lengthBins(activity: Activity, measure: LengthMeasure): HistogramBin[] {
  return activity.sessionLengths.map((b) => ({
    lower: b.lowerMs,
    upper: b.upperMs,
    count: measure === "active" ? b.byActive : b.byWall,
  }));
}

function ranked(items: readonly RankItem[], top: number): RankItem[] {
  return [...items]
    .filter((i) => i.value > 0)
    .sort((a, b) => b.value - a.value)
    .slice(0, top);
}

/** The local hours of the day with the most tokens, over every weekday. */
export function peakHours(activity: Activity, theme: ChartTheme, top = 5): RankItem[] {
  const byHour = Array.from({ length: 24 }, () => 0);
  for (const cell of activity.hours) byHour[cell.hour] = (byHour[cell.hour] ?? 0) + cell.tokens;
  const total = byHour.reduce((a, b) => a + b, 0);
  return ranked(
    byHour.map((value, hour) => ({
      key: null,
      label: `${String(hour).padStart(2, "0")}:00–${String((hour + 1) % 24).padStart(2, "0")}:00`,
      value,
      share: total === 0 ? 0 : value / total,
      color: theme.data[2] ?? theme.accent,
    })),
    top,
  );
}

/** The weekdays with the most tokens, over every hour. */
export function busiestWeekdays(activity: Activity, theme: ChartTheme): RankItem[] {
  const byDay = WEEKDAYS.map(() => 0);
  for (const cell of activity.hours) byDay[cell.weekday] = (byDay[cell.weekday] ?? 0) + cell.tokens;
  const total = byDay.reduce((a, b) => a + b, 0);
  return ranked(
    byDay.map((value, day) => ({
      key: null,
      label: WEEKDAYS[day] ?? "",
      value,
      share: total === 0 ? 0 : value / total,
      color: theme.data[3] ?? theme.accent,
    })),
    WEEKDAYS.length,
  );
}
