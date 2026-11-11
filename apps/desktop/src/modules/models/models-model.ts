/**
 * Maps the Models & Providers response to chart inputs. Pure: no React, and no formulas
 * beyond selecting and arranging what the core computed.
 */
import type { Matrix, ModelRow, Models } from "@veyra/contracts";

import {
  modelColors,
  otherColor,
  providerColor,
  type ChartTheme,
  type HeatMatrix,
  type Lane,
  type RankItem,
  type TrendSeries,
} from "@/shared/charts";

/** How many models can be compared side by side. */
export const COMPARE_MIN = 2;
export const COMPARE_MAX = 4;

/** Whether the scope has any model usage. */
export function hasModels(models: Models): boolean {
  return models.leaderboard.some((m) => m.measures.tokens > 0 || m.measures.requests > 0);
}

/** One series per provider (in its identity hue) of its daily share of tokens. */
export function shareInput(models: Models, theme: ChartTheme) {
  const series: TrendSeries[] = models.providers.map((provider, i) => ({
    name: provider.label,
    color: providerColor(theme, provider.key === "" ? null : provider.key, i),
    values: models.providerShare.map((day) => day.shares[i] ?? 0),
  }));
  return { dates: models.dates, series };
}

export function heatMatrix(matrix: Matrix): HeatMatrix {
  return {
    rows: matrix.rows.map((r) => r.label),
    columns: matrix.columns.map((c) => c.label),
    values: matrix.values,
  };
}

export function lanes(models: Models): Lane[] {
  return models.timeline.map((lane) => ({ label: lane.label, values: lane.tokens }));
}

/** A stable identity for a leaderboard row (unknown models have no key). */
export function rowId(row: ModelRow): string {
  return `${row.provider ?? ""}/${row.key ?? row.label}`;
}

/** Adds or removes a model from the comparison, never beyond the maximum. */
export function toggleCompared(selected: readonly string[], id: string): string[] {
  if (selected.includes(id)) return selected.filter((s) => s !== id);
  return selected.length >= COMPARE_MAX ? [...selected] : [...selected, id];
}

/** The measures compared side by side, each read from a leaderboard row. */
export const COMPARED: readonly {
  title: string;
  kind: "tokens" | "money" | "ratio";
  pick: (row: ModelRow) => number | null;
}[] = [
  { title: "Tokens", kind: "tokens", pick: (r) => r.measures.tokens },
  { title: "Requests", kind: "tokens", pick: (r) => r.measures.requests },
  { title: "Cost (API-equivalent)", kind: "money", pick: (r) => r.measures.costNanoUsd },
  { title: "Cost per 1M output tokens", kind: "money", pick: (r) => r.costPerMillionOutputNanoUsd },
  { title: "Cache hit ratio", kind: "ratio", pick: (r) => r.cacheHitRatio },
];

/** One measure of the compared models as ranking bars, in the order they were picked. */
export function comparedItems(
  rows: readonly ModelRow[],
  pick: (row: ModelRow) => number | null,
  theme: ChartTheme,
): RankItem[] {
  const colors = modelColors(
    theme,
    rows.map((r) => ({ id: rowId(r), provider: r.provider })),
  );
  return rows.flatMap((row) => {
    const value = pick(row);
    return value === null
      ? []
      : [
          {
            key: null,
            label: row.label,
            value,
            share: 0,
            color: colors.get(rowId(row)) ?? otherColor(theme),
          },
        ];
  });
}
