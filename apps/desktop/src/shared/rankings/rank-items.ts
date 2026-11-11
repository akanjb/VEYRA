/**
 * Turns the core's ranking rows into chart items: ordered by the chosen measure, with
 * shares of that measure and identity colours. "Other" always comes last.
 */
import type { Measures, RankRow } from "@veyra/contracts";

import { categoryColor, modelColors, otherColor, type ChartTheme, type RankItem } from "../charts";

/** Models take tints of their provider's hue; anything else takes category colours. */
export type RankColoring = "model" | "category";

export interface RankOptions {
  /** Reads the ranked measure. */
  pick: (measures: Measures) => number;
  coloring: RankColoring;
  /** Plural noun for the "Other" row, e.g. "models". */
  noun: string;
}

/** Rows by the picked measure (descending), "Other" last. */
export function sortedBy(rows: readonly RankRow[], pick: (m: Measures) => number): RankRow[] {
  const named = rows.filter((r) => !r.isOther).sort((a, b) => pick(b.measures) - pick(a.measures));
  return [...named, ...rows.filter((r) => r.isOther)];
}

/** Each row's share of the picked measure (the core's shares are of tokens). */
function shares(rows: readonly RankRow[], pick: (m: Measures) => number): number[] {
  const total = rows.reduce((sum, r) => sum + pick(r.measures), 0);
  return rows.map((r) => (total === 0 ? 0 : pick(r.measures) / total));
}

function colorsOf(
  rows: readonly RankRow[],
  coloring: RankColoring,
  theme: ChartTheme,
): (row: RankRow, index: number) => string {
  if (coloring === "category") {
    return (row, index) =>
      row.isOther || row.key === null ? otherColor(theme) : categoryColor(theme, index);
  }
  const colors = modelColors(
    theme,
    rows.filter((r) => r.key !== null).map((r) => ({ id: r.key ?? "", provider: r.provider })),
  );
  return (row) =>
    row.key === null ? otherColor(theme) : (colors.get(row.key) ?? otherColor(theme));
}

export function rankItems(
  rows: readonly RankRow[],
  options: RankOptions,
  theme: ChartTheme,
): RankItem[] {
  const ordered = sortedBy(rows, options.pick);
  const share = shares(ordered, options.pick);
  const color = colorsOf(ordered, options.coloring, theme);
  return ordered.map((r, i) => ({
    key: r.isOther ? null : r.key,
    label: r.isOther ? `Other (${String(r.groupCount)} ${options.noun})` : r.label,
    value: options.pick(r.measures),
    share: share[i] ?? 0,
    color: color(r, i),
  }));
}
