/**
 * Maps the Usage response to chart inputs. Pure: no React, and no formulas beyond
 * selecting and arranging what the core computed.
 */
import type { Breakdown, BreakdownDimension, SizePoint, Usage } from "@veyra/contracts";

import type { BandPoint, ChartTheme, HistogramBin, RankItem, TrendSeries } from "@/shared/charts";
import type { DimensionKey } from "@/shared/filters";
import { rankItems, type RankColoring } from "@/shared/rankings";

/** How each breakdown is labelled, coloured and (where possible) filtered. */
export const BREAKDOWN: Readonly<
  Record<
    BreakdownDimension,
    { label: string; noun: string; coloring: RankColoring; filter: DimensionKey | null }
  >
> = {
  model: { label: "Model", noun: "models", coloring: "model", filter: "models" },
  provider: { label: "Provider", noun: "providers", coloring: "category", filter: "providers" },
  app: { label: "Application", noun: "applications", coloring: "category", filter: "apps" },
  interface: { label: "Interface", noun: "interfaces", coloring: "category", filter: "interfaces" },
  project: { label: "Project", noun: "projects", coloring: "category", filter: "projects" },
  device: { label: "Device", noun: "devices", coloring: "category", filter: null },
};

/** Whether the scope has any recorded usage. */
export function hasUsage(usage: Usage): boolean {
  return usage.composition.some((d) => d.inputUncached + d.cacheRead + d.cacheWrite + d.output > 0);
}

/** The token kinds stacked per day, with reasoning as an overlay where reported. */
export function compositionInput(usage: Usage, theme: ChartTheme) {
  const days = usage.composition;
  const color = (slot: number) => theme.data[slot] ?? theme.accent;
  const series: TrendSeries[] = [
    { name: "Input (uncached)", color: color(2), values: days.map((d) => d.inputUncached) },
    { name: "Cache reads", color: color(6), values: days.map((d) => d.cacheRead) },
    { name: "Cache writes", color: color(4), values: days.map((d) => d.cacheWrite) },
    { name: "Output", color: color(3), values: days.map((d) => d.output) },
  ];
  const overlay: TrendSeries | undefined = usage.reasoningReported
    ? { name: "Reasoning (within output)", color: theme.text, values: days.map((d) => d.reasoning) }
    : undefined;
  return { dates: days.map((d) => d.date), series, overlay };
}

/** The breakdown of one dimension. */
export function breakdownOf(usage: Usage, dimension: BreakdownDimension): Breakdown | undefined {
  return usage.breakdowns.find((b) => b.dimension === dimension);
}

/** A breakdown's rows as chart items, in the core's order (by tokens, "Other" last). */
export function breakdownItems(breakdown: Breakdown, theme: ChartTheme): RankItem[] {
  const meta = BREAKDOWN[breakdown.dimension];
  return rankItems(
    breakdown.rows,
    { pick: (m) => m.tokens, coloring: meta.coloring, noun: meta.noun },
    theme,
  );
}

export function bandPoints(points: readonly SizePoint[]): BandPoint[] {
  return points.map((p) => ({
    date: p.date,
    count: p.count,
    mean: p.mean,
    p50: p.p50,
    p90: p.p90,
  }));
}

export function histogramBins(usage: Usage): HistogramBin[] {
  return usage.requestSizes.map((b) => ({ lower: b.lower, upper: b.upper, count: b.requests }));
}
