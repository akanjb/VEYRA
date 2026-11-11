/**
 * Maps the Cache response to chart inputs. Pure: no React, and no formulas beyond
 * selecting and arranging what the core computed.
 */
import type { Cache, RatioRow } from "@veyra/contracts";

import {
  categoryColor,
  modelColors,
  otherColor,
  type ChartTheme,
  type GroupSeries,
  type MirroredSeries,
  type RankItem,
} from "@/shared/charts";
import { fmt } from "@/shared/format";

/** Whether the scope has any cache activity. */
export function hasCacheUse(cache: Cache): boolean {
  return cache.daily.some((d) => d.reads > 0 || d.writes > 0);
}

/** Cache reads above the axis and cache writes below it, per day. */
export function flowInput(cache: Cache, theme: ChartTheme) {
  const reads: MirroredSeries = {
    name: "Cache reads",
    color: theme.data[6] ?? theme.accent,
    values: cache.daily.map((d) => d.reads),
  };
  const writes: MirroredSeries = {
    name: "Cache writes",
    color: theme.data[4] ?? theme.accent,
    values: cache.daily.map((d) => d.writes),
  };
  return { dates: cache.daily.map((d) => d.date), reads, writes };
}

/**
 * Hit-ratio bars for the groups that report one, in the core's order (most input
 * first). Models take their provider's hue; other groups take category colours.
 */
export function ratioItems(
  rows: readonly RatioRow[],
  coloring: "model" | "category",
  theme: ChartTheme,
): RankItem[] {
  const models = modelColors(
    theme,
    rows.filter((r) => r.key !== null).map((r) => ({ id: r.key ?? "", provider: r.provider })),
  );
  const color = (row: RatioRow, index: number) => {
    if (row.isOther || row.key === null) return otherColor(theme);
    return coloring === "model"
      ? (models.get(row.key) ?? otherColor(theme))
      : categoryColor(theme, index);
  };
  return rows.flatMap((row, i) =>
    row.hitRatio === null
      ? []
      : [
          {
            key: row.isOther ? null : row.key,
            label: row.label,
            value: row.hitRatio,
            share: row.hitRatio,
            color: color(row, i),
            detail: `${fmt.compact(row.cacheRead)} of ${fmt.compact(row.inputTokens)} input tokens`,
          },
        ],
  );
}

/** Writes per lifetime and reads, side by side per model. */
export function ttlInput(cache: Cache, theme: ChartTheme) {
  const rows = cache.writesByModel;
  const color = (slot: number) => theme.data[slot] ?? theme.accent;
  const series: GroupSeries[] = [
    { name: "5-minute writes", color: color(4), values: rows.map((r) => r.short) },
    { name: "1-hour writes", color: color(8), values: rows.map((r) => r.long) },
    {
      name: "Writes, lifetime not reported",
      color: color(5),
      values: rows.map((r) => r.unspecified),
    },
    { name: "Cache reads", color: color(6), values: rows.map((r) => r.reads) },
  ];
  // Drop the columns no model has, so the chart shows only what was reported.
  const shown = series.filter((s, i) => i === series.length - 1 || s.values.some((v) => v > 0));
  return { categories: rows.map((r) => r.label), series: shown };
}
