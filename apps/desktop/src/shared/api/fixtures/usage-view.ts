/**
 * The Usage response built from the synthetic dataset, shaped like the core's. Only for
 * development and UI tests: product numbers always come from the Rust analytics engine.
 */
import type {
  Breakdown,
  BreakdownDimension,
  RankRow,
  Scope,
  SizeBinCount,
  SizePoint,
  Usage,
} from "@veyra/contracts";

import {
  kpi,
  localDay,
  measuresOf,
  previousRange,
  rank,
  select,
  tokensOf,
  type Flat,
} from "./aggregate";
import { FIXTURE_PROJECTS, fixtureLabel, type FixtureEvent } from "./dataset";
import { daysOf, groupBy } from "./views";

const TOP_ROWS = 10;

function quantile(sorted: readonly number[], p: number): number | null {
  if (sorted.length === 0) return null;
  const position = p * (sorted.length - 1);
  const below = sorted[Math.floor(position)] ?? 0;
  const above = sorted[Math.ceil(position)] ?? 0;
  return below + (above - below) * (position - Math.floor(position));
}

function sizePoint(date: string, values: readonly number[]): SizePoint {
  const sorted = [...values].sort((a, b) => a - b);
  return {
    date,
    count: sorted.length,
    mean: sorted.length === 0 ? null : sorted.reduce((a, b) => a + b, 0) / sorted.length,
    p50: quantile(sorted, 0.5),
    p90: quantile(sorted, 0.9),
  };
}

/** Quarter-doubling size bins, like the core's. */
function sizeBins(sizes: readonly number[]): SizeBinCount[] {
  const counts = new Map<number, number>();
  for (const size of sizes) {
    const bin = size === 0 ? 0 : 1 + Math.floor(4 * Math.log2(size));
    counts.set(bin, (counts.get(bin) ?? 0) + 1);
  }
  const lower = (bin: number) => (bin === 0 ? 0 : Math.ceil(2 ** ((bin - 1) / 4)));
  return [...counts.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([bin, requests]) => ({ lower: lower(bin), upper: lower(bin + 1), requests }));
}

const KEYS: Readonly<
  Record<BreakdownDimension, (f: Flat) => { key: string; label: string; provider: string | null }>
> = {
  model: (f) => ({
    key: f.event.model,
    label: fixtureLabel(f.event.model).label,
    provider: f.event.provider,
  }),
  provider: (f) => ({
    key: f.event.provider,
    label: fixtureLabel(f.event.provider).label,
    provider: null,
  }),
  app: (f) => ({ key: f.session.app, label: fixtureLabel(f.session.app).label, provider: null }),
  interface: (f) => ({
    key: f.session.interface,
    label: f.session.interface.toUpperCase(),
    provider: null,
  }),
  project: (f) => ({
    key: String(f.session.projectId),
    label: FIXTURE_PROJECTS[f.session.projectId - 1] ?? "No project",
    provider: null,
  }),
  device: () => ({ key: "", label: "This device", provider: null }),
};

function breakdown(
  selected: readonly Flat[],
  dimension: BreakdownDimension,
  tokens: number,
): Breakdown {
  const describe = KEYS[dimension];
  const groups = groupBy(
    selected,
    (f) => describe(f).key,
    (f) => describe(f),
  );
  const rows: RankRow[] = rank(groups, TOP_ROWS, tokens).map((r) =>
    dimension === "device" ? { ...r, key: null } : r,
  );
  const totals = [...groups.values()].map((g) => measuresOf(g.events).tokens);
  let running = 0;
  return {
    dimension,
    rows,
    cumulativeShare: rows.map((r) => {
      running += r.measures.tokens;
      return tokens === 0 ? 0 : running / tokens;
    }),
    concentration: tokens === 0 ? null : totals.reduce((sum, t) => sum + (t / tokens) ** 2, 0),
  };
}

function headline(period: readonly Flat[]) {
  const events = period.map((f) => f.event);
  const sum = (pick: (e: FixtureEvent) => number) => events.reduce((n, e) => n + pick(e), 0);
  const reported = events.length > 0;
  const tokens = sum(tokensOf);
  return {
    tokens: reported ? tokens : null,
    input: reported ? sum((e) => e.input + e.cacheRead + e.cacheWrite) : null,
    output: reported ? sum((e) => e.output) : null,
    reasoning: reported ? sum((e) => e.reasoning) : null,
    requests: reported ? events.length : null,
    perRequest: reported ? tokens / events.length : null,
  };
}

/** Each session's tokens, on the local day of its first request. */
function sessionsByFirstDay(selected: readonly Flat[], dayOf: (f: Flat) => string) {
  const sessions = new Map<number, { day: string; tokens: number }>();
  for (const f of [...selected].sort((a, b) => a.event.at - b.event.at)) {
    const entry = sessions.get(f.session.id) ?? { day: dayOf(f), tokens: 0 };
    entry.tokens += tokensOf(f.event);
    sessions.set(f.session.id, entry);
  }
  return [...sessions.values()];
}

function mixPoint(date: string, events: readonly FixtureEvent[]) {
  const sum = (pick: (e: FixtureEvent) => number) => events.reduce((n, e) => n + pick(e), 0);
  return {
    date,
    inputUncached: sum((e) => e.input),
    cacheRead: sum((e) => e.cacheRead),
    cacheWrite: sum((e) => e.cacheWrite),
    output: sum((e) => e.output),
    reasoning: events.length === 0 ? null : sum((e) => e.reasoning),
  };
}

function kpis(selected: readonly Flat[], before: readonly Flat[] | null): Usage["kpis"] {
  const now = headline(selected);
  const then = before === null ? null : headline(before);
  const tile = (key: keyof typeof now, definition: string) =>
    kpi(now[key], then?.[key] ?? null, definition);
  return {
    tokens: tile("tokens", "Fixture: all prompt and output tokens."),
    inputTokens: tile("input", "Fixture: prompt tokens."),
    outputTokens: tile("output", "Fixture: generated tokens."),
    reasoningTokens: tile("reasoning", "Fixture: reasoning tokens."),
    requests: tile("requests", "Fixture: model API calls."),
    tokensPerRequest: { ...tile("perRequest", "Fixture: tokens per request."), accuracy: null },
  };
}

const DIMENSIONS: readonly BreakdownDimension[] = [
  "model",
  "provider",
  "app",
  "interface",
  "project",
  "device",
];

export function usage(flat: readonly Flat[], scope: Scope): Usage {
  const selected = select(flat, scope, scope.start, scope.end);
  const prev = previousRange(scope.start, scope.end);
  const before = scope.comparison === "previous" ? select(flat, scope, prev.start, prev.end) : null;
  const days = daysOf(scope.start, scope.end);
  const dayOf = (f: Flat) => localDay(f.event.at, scope.timeZone);
  const byDay = groupBy(selected, dayOf, () => ({ label: "", provider: null }));
  const eventsOn = (date: string) => byDay.get(date)?.events ?? [];
  const sessions = sessionsByFirstDay(selected, dayOf);
  const tokens = measuresOf(selected.map((f) => f.event)).tokens;
  return {
    kpis: kpis(selected, before),
    accuracy: selected.length > 0 ? "exact" : null,
    composition: days.map((date) => mixPoint(date, eventsOn(date))),
    reasoningReported: selected.length > 0,
    breakdowns: DIMENSIONS.map((d) => breakdown(selected, d, tokens)),
    perRequest: days.map((date) => sizePoint(date, eventsOn(date).map(tokensOf))),
    perSession: days.map((date) =>
      sizePoint(
        date,
        sessions.filter((s) => s.day === date).map((s) => s.tokens),
      ),
    ),
    requestSizes: sizeBins(selected.map((f) => tokensOf(f.event))),
    requestDefinition: "Fixture: median and 90th percentile of tokens per request.",
    sessionDefinition: "Fixture: tokens per session.",
    concentrationDefinition: "Fixture: the sum of squared shares.",
  };
}
