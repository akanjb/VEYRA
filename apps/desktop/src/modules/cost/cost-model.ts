/**
 * Maps the Cost response to chart inputs. Pure: no React, and no formulas beyond
 * selecting and arranging what the core computed. The three cost kinds are only ever
 * shown side by side; nothing here adds one kind to another (ADR-004).
 */
import type {
  BreakdownDimension,
  BudgetLevel,
  BudgetStatus,
  Cost,
  CostBreakdown,
  CostKpis,
  Waterfall,
} from "@veyra/contracts";

import {
  modelColors,
  otherColor,
  type ChartTheme,
  type LineSeries,
  type RankItem,
  type SharePart,
  type ShareRow,
  type Threshold,
  type TrendSeries,
  type WaterfallBar,
} from "@/shared/charts";
import type { DimensionKey } from "@/shared/filters";
import { rankItems, type RankColoring } from "@/shared/rankings";

/** How each tile reads and formats its number. */
export type TileFormat = "estimate" | "recorded" | "ratio";

export const TILES: readonly { key: keyof CostKpis; label: string; format: TileFormat }[] = [
  { key: "apiEquivalentNanoUsd", label: "API-equivalent cost", format: "estimate" },
  { key: "subscriptionSpendNanoUsd", label: "Subscription spend", format: "recorded" },
  { key: "actualChargeNanoUsd", label: "Actual charges", format: "recorded" },
  { key: "pricingCoverage", label: "Pricing coverage", format: "ratio" },
  { key: "costPerRequestNanoUsd", label: "Cost per request", format: "estimate" },
  { key: "costPerMillionOutputNanoUsd", label: "Cost per 1M output", format: "estimate" },
];

/** How each breakdown is labelled, coloured and filtered. */
export const BREAKDOWN: Readonly<
  Partial<
    Record<
      BreakdownDimension,
      { label: string; noun: string; coloring: RankColoring; filter: DimensionKey }
    >
  >
> = {
  model: { label: "Model", noun: "models", coloring: "model", filter: "models" },
  provider: { label: "Provider", noun: "providers", coloring: "category", filter: "providers" },
  app: { label: "Application", noun: "applications", coloring: "category", filter: "apps" },
  project: { label: "Project", noun: "projects", coloring: "category", filter: "projects" },
};

/** Whether the scope has any API-equivalent cost. */
export function hasCost(cost: Cost): boolean {
  return cost.daily.some((d) => d.apiEquivalentNanoUsd !== 0);
}

/**
 * The value framing (03-ux.md §4): subscription spend beside what the same usage would
 * cost at API rates, as a comparison and never as a total. `null` without subscriptions.
 */
export function valueFraming(cost: Cost): { subscription: number; apiEquivalent: number } | null {
  const subscription = cost.kpis.subscriptionSpendNanoUsd.value ?? 0;
  if (subscription <= 0) return null;
  return { subscription, apiEquivalent: cost.kpis.apiEquivalentNanoUsd.value ?? 0 };
}

/** Daily cost bars with the running total on its own axis. */
export function dailyInput(cost: Cost, theme: ChartTheme) {
  const bars: TrendSeries = {
    name: "API-equivalent cost",
    color: theme.data[0] ?? theme.accent,
    values: cost.daily.map((d) => d.apiEquivalentNanoUsd),
  };
  const running: TrendSeries = {
    name: "Running total",
    color: theme.text,
    values: cost.daily.map((d) => d.cumulativeNanoUsd),
  };
  return { dates: cost.daily.map((d) => d.date), series: [bars], overlay: running };
}

/** The running total of this period and of the previous one, day by day. */
export function cumulativeInput(cost: Cost, theme: ChartTheme) {
  const series: LineSeries[] = [
    {
      name: "This period",
      color: theme.data[0] ?? theme.accent,
      values: cost.daily.map((d) => d.cumulativeNanoUsd),
    },
    {
      name: "Previous period",
      color: theme.subtle,
      values: cost.daily.map((d) => d.previousCumulativeNanoUsd),
      dashed: true,
    },
  ];
  return { dates: cost.daily.map((d) => d.date), series };
}

/** Budgets that cover all usage, drawn as threshold lines on the month outlook. */
function budgetLines(budgets: readonly BudgetStatus[], theme: ChartTheme): Threshold[] {
  return budgets
    .filter((b) => b.project === null)
    .map((b) => ({ label: b.label, value: b.limitNanoUsd, color: theme.danger }));
}

/** Month to date (solid), the projection to month end (dashed) and the budget lines. */
export function outlookInput(cost: Cost, theme: ChartTheme) {
  const { month } = cost;
  const series: LineSeries[] = [
    {
      name: "Month to date",
      color: theme.data[0] ?? theme.accent,
      values: month.days.map((d) => d.cumulativeNanoUsd),
    },
    {
      name: "Projection",
      color: theme.data[0] ?? theme.accent,
      values: month.days.map((d) => d.projectedNanoUsd),
      dashed: true,
    },
  ];
  return {
    dates: month.days.map((d) => d.date),
    series,
    thresholds: budgetLines(cost.budgets, theme),
  };
}

export function breakdownOf(cost: Cost, dimension: BreakdownDimension): CostBreakdown | undefined {
  return cost.breakdowns.find((b) => b.dimension === dimension);
}

/** A breakdown's rows as chart items by cost, "Other" last. */
export function breakdownItems(breakdown: CostBreakdown, theme: ChartTheme): RankItem[] {
  const meta = BREAKDOWN[breakdown.dimension];
  return rankItems(
    breakdown.rows,
    {
      pick: (m) => m.costNanoUsd,
      coloring: meta?.coloring ?? "category",
      noun: meta?.noun ?? "groups",
    },
    theme,
  );
}

/** The token kinds cost divides into, in the Usage page's colours. */
export function compositionParts(theme: ChartTheme): SharePart[] {
  const color = (slot: number) => theme.data[slot] ?? theme.accent;
  return [
    { name: "Input (uncached)", color: color(2) },
    { name: "Cache reads", color: color(6) },
    { name: "Cache writes", color: color(4) },
    { name: "Output", color: color(3) },
  ];
}

/** One row per group: its cost by token kind, aligned with {@link compositionParts}. */
export function compositionRows(cost: Cost): ShareRow[] {
  return cost.composition.map((m) => ({
    label: m.label,
    amounts: [m.inputNanoUsd, m.cacheReadNanoUsd, m.cacheWriteNanoUsd, m.outputNanoUsd],
  }));
}

/** Effective rates as dots, in the core's order (highest first), in model colours. */
export function rateItems(cost: Cost, theme: ChartTheme): RankItem[] {
  const colors = modelColors(
    theme,
    cost.rates
      .filter((r) => r.key !== null)
      .map((r) => ({ id: r.key ?? "", provider: r.provider })),
  );
  return cost.rates.map((r) => ({
    key: r.key,
    label: r.label,
    value: r.costPerMillionOutputNanoUsd,
    share: 0,
    color: (r.key === null ? undefined : colors.get(r.key)) ?? otherColor(theme),
  }));
}

/** The previous total, each change, and the current total. */
export function waterfallBars(waterfall: Waterfall, theme: ChartTheme): WaterfallBar[] {
  const rise = theme.data[0] ?? theme.accent;
  const fall = theme.data[1] ?? theme.accent;
  return [
    {
      label: "Previous period",
      value: waterfall.previousNanoUsd,
      kind: "total",
      color: theme.subtle,
    },
    ...waterfall.steps.map((s) => ({
      label: s.isOther ? `Other (${String(s.groupCount)})` : s.label,
      value: s.changeNanoUsd,
      kind: "change" as const,
      color: s.changeNanoUsd >= 0 ? rise : fall,
    })),
    { label: "This period", value: waterfall.currentNanoUsd, kind: "total", color: theme.text },
  ];
}

/** How far a budget is used (0–1, capped at 1 for the meter). */
export function budgetUse(budget: BudgetStatus): number {
  if (budget.limitNanoUsd <= 0) return 0;
  return Math.min(1, budget.monthToDateNanoUsd / budget.limitNanoUsd);
}

/** The words for a budget level; colour is never the only signal. */
export const LEVEL_LABEL: Readonly<Record<BudgetLevel, string>> = {
  within: "Within budget",
  warning: "Near the limit",
  over: "Over budget",
};
