/**
 * The Cost response built from the synthetic dataset, shaped like the core's. Only for
 * development and UI tests: product numbers always come from the Rust analytics engine.
 */
import type {
  BreakdownDimension,
  BudgetLevel,
  Cost,
  CostMix,
  Scope,
  Waterfall,
  WaterfallStep,
} from "@veyra/contracts";

import { addDays, monthStart, todayIn } from "../../filters/dates";
import { kpi, localDay, measuresOf, previousRange, rank, select, type Flat } from "./aggregate";
import type { FixtureBilling } from "./billing-view";
import type { FixtureEvent } from "./dataset";
import { KEYS } from "./usage-view";
import { daysOf, groupBy, sessions } from "./views";

const TOP_ROWS = 10;
const STEPS = 8;
const BREAKDOWNS: readonly BreakdownDimension[] = ["model", "provider", "app", "project"];
const cost = (events: readonly FixtureEvent[]) => measuresOf(events).costNanoUsd;
const tokens = (e: FixtureEvent) => e.input + e.cacheRead + e.cacheWrite + e.output;

function dailyCost(flat: readonly Flat[], scope: Scope, days: readonly string[]): number[] {
  const byDay = new Map<string, number>();
  for (const f of flat) {
    const day = localDay(f.event.at, scope.timeZone);
    byDay.set(day, (byDay.get(day) ?? 0) + f.event.costNanoUsd);
  }
  return days.map((d) => byDay.get(d) ?? 0);
}

function running(values: readonly number[]): number[] {
  let total = 0;
  return values.map((v) => (total += v));
}

/** Each event's cost split over its token kinds in proportion to the tokens. */
function mix(label: string, key: string | null, provider: string | null, events: FixtureEvent[]) {
  const part = (pick: (e: FixtureEvent) => number) =>
    Math.round(
      events.reduce((n, e) => n + (tokens(e) === 0 ? 0 : (e.costNanoUsd * pick(e)) / tokens(e)), 0),
    );
  return {
    key,
    label,
    provider,
    inputNanoUsd: part((e) => e.input),
    cacheReadNanoUsd: part((e) => e.cacheRead),
    cacheWriteNanoUsd: part((e) => e.cacheWrite),
    outputNanoUsd: part((e) => e.output),
  } satisfies CostMix;
}

function waterfall(
  dimension: BreakdownDimension,
  current: readonly Flat[],
  previous: readonly Flat[],
): Waterfall {
  const describe = KEYS[dimension];
  const values = new Map<
    string,
    { label: string; provider: string | null; now: number; before: number }
  >();
  const add = (list: readonly Flat[], field: "now" | "before") => {
    for (const f of list) {
      const d = describe(f);
      const entry = values.get(d.key) ?? {
        label: d.label,
        provider: d.provider,
        now: 0,
        before: 0,
      };
      entry[field] += f.event.costNanoUsd;
      values.set(d.key, entry);
    }
  };
  add(current, "now");
  add(previous, "before");
  const changes = [...values.entries()]
    .map(([key, v]) => ({ key, ...v, change: v.now - v.before }))
    .filter((c) => c.change !== 0)
    .sort((a, b) => Math.abs(b.change) - Math.abs(a.change) || a.key.localeCompare(b.key));
  const steps: WaterfallStep[] = changes.slice(0, STEPS).map((c) => ({
    key: c.key,
    label: c.label,
    provider: c.provider,
    changeNanoUsd: c.change,
    groupCount: 1,
    isOther: false,
  }));
  const rest = changes.slice(STEPS);
  if (rest.length > 0)
    steps.push({
      key: null,
      label: "Other",
      provider: null,
      changeNanoUsd: rest.reduce((n, c) => n + c.change, 0),
      groupCount: rest.length,
      isOther: true,
    });
  return {
    dimension,
    previousNanoUsd: cost(previous.map((f) => f.event)),
    currentNanoUsd: cost(current.map((f) => f.event)),
    steps,
  };
}

function level(value: number, limit: number, warn: number): BudgetLevel {
  if (value >= limit) return "over";
  return value * 100 >= limit * warn ? "warning" : "within";
}

function outlook(flat: readonly Flat[], scope: Scope, now: number) {
  const first = monthStart(scope.end);
  const days = daysOf(first, addDays(monthStart(addDays(first, 40)), -1));
  const today = todayIn(scope.timeZone, new Date(now));
  const elapsed = days.filter((d) => d <= today).length;
  const daily = dailyCost(select(flat, scope, first, days.at(-1) ?? first), scope, days);
  const monthToDate = daily.slice(0, elapsed).reduce((a, b) => a + b, 0);
  const until = today < (days.at(-1) ?? today) ? today : addDays(days.at(-1) ?? today, 1);
  const recent = dailyCost(
    select(flat, scope, addDays(until, -7), addDays(until, -1)),
    scope,
    daysOf(addDays(until, -7), addDays(until, -1)),
  );
  const runRate = Math.round(recent.reduce((a, b) => a + b, 0) / 7);
  const daysLeft = days.length - elapsed;
  const totals = running(daily);
  return {
    month: first.slice(0, 7),
    asOf: days[Math.max(0, elapsed - 1)] ?? first,
    days: days.map((date, i) => ({
      date,
      cumulativeNanoUsd: i < elapsed ? (totals[i] ?? 0) : null,
      projectedNanoUsd:
        i >= elapsed
          ? monthToDate + runRate * (i - elapsed + 1)
          : i === elapsed - 1
            ? monthToDate
            : null,
    })),
    monthToDateNanoUsd: monthToDate,
    runRateNanoUsd: runRate,
    forecastNanoUsd: monthToDate + runRate * daysLeft,
    daysLeft,
  };
}

/** The selections a Cost response is built from. */
interface Periods {
  selected: Flat[];
  previous: Flat[];
  days: string[];
  previousDays: string[];
  compared: boolean;
}

function periodsOf(flat: readonly Flat[], scope: Scope): Periods {
  const prev = previousRange(scope.start, scope.end);
  return {
    selected: select(flat, scope, scope.start, scope.end),
    previous: select(flat, scope, prev.start, prev.end),
    days: daysOf(scope.start, scope.end),
    previousDays: daysOf(prev.start, prev.end),
    compared: scope.comparison === "previous",
  };
}

function costKpis(p: Periods, billing: FixtureBilling): Cost["kpis"] {
  const events = p.selected.map((f) => f.event);
  const total = cost(events);
  const tile = (value: number | null, before: number | null, definition: string) =>
    kpi(value, p.compared ? before : null, definition);
  const recorded = (kind: "charge" | "subscription", definition: string) => ({
    ...tile(billing.total(kind, p.days), billing.total(kind, p.previousDays), definition),
    accuracy: null,
  });
  return {
    apiEquivalentNanoUsd: {
      ...tile(total, cost(p.previous.map((f) => f.event)), "Fixture: API-equivalent cost."),
      accuracy: "estimated",
    },
    pricingCoverage: tile(
      events.length === 0 ? null : 1,
      p.previous.length === 0 ? null : 1,
      "Fixture: priced tokens.",
    ),
    actualChargeNanoUsd: recorded("charge", "Fixture: recorded charges."),
    subscriptionSpendNanoUsd: recorded("subscription", "Fixture: recorded subscriptions."),
    costPerRequestNanoUsd: tile(
      events.length === 0 ? null : Math.round(total / events.length),
      null,
      "Fixture: cost per request.",
    ),
    costPerMillionOutputNanoUsd: tile(null, null, "Fixture: cost per million output tokens."),
  };
}

function budgetRows(flat: readonly Flat[], scope: Scope, billing: FixtureBilling, now: number) {
  const unfiltered = {
    ...scope,
    filters: { providers: [], models: [], apps: [], interfaces: [], projects: [] },
  };
  const month = outlook(flat, unfiltered, now);
  return billing.budgets().map(({ id, budget }) => ({
    id,
    label: budget.label,
    project: null,
    limitNanoUsd: budget.limitNanoUsd,
    warnPercent: budget.warnPercent,
    monthToDateNanoUsd: month.monthToDateNanoUsd,
    forecastNanoUsd: month.forecastNanoUsd,
    level: level(month.monthToDateNanoUsd, budget.limitNanoUsd, budget.warnPercent),
    forecastLevel: level(month.forecastNanoUsd, budget.limitNanoUsd, budget.warnPercent),
  }));
}

/** Cost by token kind and effective rates, for the priced models. */
function byModel(selected: readonly Flat[]) {
  const models = groupBy(
    selected,
    (f) => f.event.model,
    (f) => KEYS.model(f),
  );
  const priced = [...models.entries()]
    .filter(([, g]) => cost(g.events) > 0)
    .sort((a, b) => cost(b[1].events) - cost(a[1].events));
  const composition = [
    mix(
      "All models",
      null,
      null,
      selected.map((f) => f.event),
    ),
    ...priced.slice(0, 8).map(([key, g]) => mix(g.label, key, g.provider, g.events)),
  ];
  const rates = priced
    .map(([key, g]) => {
      const output = g.events.reduce((n, e) => n + e.output, 0);
      return {
        key,
        label: g.label,
        provider: g.provider,
        costPerMillionOutputNanoUsd: output === 0 ? 0 : Math.round((cost(g.events) * 1e6) / output),
      };
    })
    .filter((r) => r.costPerMillionOutputNanoUsd > 0)
    .sort((a, b) => b.costPerMillionOutputNanoUsd - a.costPerMillionOutputNanoUsd);
  return { composition, rates };
}

const DEFINITIONS: Cost["definitions"] = {
  runRate: "Fixture: the last 7 complete days divided by 7.",
  forecast: "Fixture: a projection of the month.",
  budget: "Fixture: a monthly limit on API-equivalent cost.",
  waterfall: "Fixture: change from the previous period.",
  subscriptions: "Fixture: recorded subscriptions.",
  charges: "Fixture: recorded charges.",
};

export function costView(
  flat: readonly Flat[],
  scope: Scope,
  billing: FixtureBilling,
  now: number,
): Cost {
  const p = periodsOf(flat, scope);
  const total = cost(p.selected.map((f) => f.event));
  const daily = dailyCost(p.selected, scope, p.days);
  const totals = running(daily);
  const previousRunning = running(dailyCost(p.previous, scope, p.previousDays));
  return {
    kpis: costKpis(p, billing),
    accuracy: p.selected.length > 0 ? "exact" : null,
    otherCurrencyEntries: billing.otherCurrencyEntries(),
    daily: p.days.map((date, i) => ({
      date,
      apiEquivalentNanoUsd: daily[i] ?? 0,
      cumulativeNanoUsd: totals[i] ?? 0,
      previousCumulativeNanoUsd: previousRunning[i] ?? 0,
    })),
    month: outlook(flat, scope, now),
    budgets: budgetRows(flat, scope, billing, now),
    breakdowns: BREAKDOWNS.map((dimension) => ({
      dimension,
      rows: rank(
        groupBy(
          p.selected,
          (f) => KEYS[dimension](f).key,
          (f) => KEYS[dimension](f),
        ),
        TOP_ROWS,
        total,
        (m) => m.costNanoUsd,
      ),
    })),
    sessions: sessions(flat, { scope, sort: "cost", page: 0, pageSize: 10 }).rows,
    ...byModel(p.selected),
    waterfalls: [
      waterfall("model", p.selected, p.previous),
      waterfall("project", p.selected, p.previous),
    ],
    definitions: DEFINITIONS,
  };
}
