/**
 * The Models & Providers response built from the synthetic dataset, shaped like the
 * core's. Only for development and UI tests: product numbers always come from the Rust
 * analytics engine.
 */
import type { Labelled, Matrix, ModelRow, Models, Scope } from "@veyra/contracts";

import { localDay, measuresOf, select, tokensOf, type Flat } from "./aggregate";
import { fixtureLabel, type FixtureEvent } from "./dataset";
import { daysOf, groupBy } from "./views";

const TOP_MODELS = 12;

/** Keys by total tokens, largest first. */
function ranked(groups: Map<string, { events: FixtureEvent[] }>): string[] {
  return [...groups.entries()]
    .sort((a, b) => measuresOf(b[1].events).tokens - measuresOf(a[1].events).tokens)
    .map(([key]) => key);
}

function matrix(
  selected: readonly Flat[],
  rowOf: (f: Flat) => string,
  rows: readonly string[],
  columns: readonly string[],
): Matrix {
  const values = rows.map(() => columns.map(() => 0));
  for (const f of selected) {
    const r = rows.indexOf(rowOf(f));
    const c = columns.indexOf(f.session.app);
    const row = values[r];
    if (row !== undefined && c >= 0) row[c] = (row[c] ?? 0) + tokensOf(f.event);
  }
  return {
    rows: rows.map(fixtureLabel),
    columns: columns.map(fixtureLabel),
    values,
  };
}

function modelRow(
  key: string,
  events: readonly FixtureEvent[],
  dayOf: (at: number) => string,
): ModelRow {
  const m = measuresOf(events);
  const output = events.reduce((n, e) => n + e.output, 0);
  const input = events.reduce((n, e) => n + e.input + e.cacheRead + e.cacheWrite, 0);
  const read = events.reduce((n, e) => n + e.cacheRead, 0);
  const days = events.map((e) => dayOf(e.at)).sort();
  const provider = events[0]?.provider ?? null;
  return {
    key,
    label: fixtureLabel(key).label,
    provider,
    providerLabel: provider === null ? "Unknown provider" : fixtureLabel(provider).label,
    measures: m,
    costPerMillionOutputNanoUsd: output === 0 ? null : Math.round((m.costNanoUsd * 1e6) / output),
    cacheHitRatio: input === 0 ? null : read / input,
    pricingCoverage: 1,
    firstDay: days[0] ?? "",
    lastDay: days.at(-1) ?? "",
  };
}

export function models(flat: readonly Flat[], scope: Scope): Models {
  const selected = select(flat, scope, scope.start, scope.end);
  const dates = daysOf(scope.start, scope.end);
  // Each event's local day, computed once: formatting a date is the expensive part.
  const days = new Map(selected.map((f) => [f.event.at, localDay(f.event.at, scope.timeZone)]));
  const dayOf = (at: number) => days.get(at) ?? localDay(at, scope.timeZone);
  const none = () => ({ label: "", provider: null });
  const byModel = groupBy(selected, (f) => f.event.model, none);
  const byProvider = groupBy(selected, (f) => f.event.provider, none);
  const byApp = groupBy(selected, (f) => f.session.app, none);
  const modelKeys = ranked(byModel);
  const providerKeys = ranked(byProvider);
  const appKeys = ranked(byApp);
  const top = modelKeys.slice(0, TOP_MODELS);
  const perDay = (events: readonly FixtureEvent[]) => {
    const totals = new Map<string, number>();
    for (const e of events) totals.set(dayOf(e.at), (totals.get(dayOf(e.at)) ?? 0) + tokensOf(e));
    return dates.map((date) => totals.get(date) ?? 0);
  };
  const providerDays = providerKeys.map((key) => perDay(byProvider.get(key)?.events ?? []));
  const labelled = (keys: readonly string[]): Labelled[] => keys.map(fixtureLabel);
  return {
    accuracy: selected.length > 0 ? "exact" : null,
    dates,
    providers: labelled(providerKeys),
    providerShare: dates.map((date, day) => {
      const total = providerDays.reduce((n, series) => n + (series[day] ?? 0), 0);
      return {
        date,
        shares: providerDays.map((series) => (total === 0 ? 0 : (series[day] ?? 0) / total)),
      };
    }),
    leaderboard: modelKeys.map((key) => modelRow(key, byModel.get(key)?.events ?? [], dayOf)),
    apps: labelled(appKeys),
    providerByApp: matrix(selected, (f) => f.event.provider, providerKeys, appKeys),
    modelByApp: matrix(selected, (f) => f.event.model, top, appKeys),
    timeline: top.map((key) => ({
      key,
      label: fixtureLabel(key).label,
      provider: byModel.get(key)?.events[0]?.provider ?? null,
      tokens: perDay(byModel.get(key)?.events ?? []),
    })),
    rateDefinition: "Fixture: cost per million output tokens.",
  };
}
