/** Responses built from the synthetic dataset, shaped like the core's. */
import type {
  CalendarDay,
  DailyPoint,
  HourCell,
  Labelled,
  Overview,
  Scope,
  SessionList,
  SessionsRequest,
} from "@veyra/contracts";

import { addDays } from "../../filters/dates";
import {
  activity,
  kpi,
  localDay,
  localHourWeekday,
  measuresOf,
  previousRange,
  rank,
  select,
  sessionRow,
  tokensOf,
  type Flat,
} from "./aggregate";
import { FIXTURE_PROJECTS, fixtureLabel, type FixtureEvent } from "./dataset";

/** Keys with their display names, sorted by name (like the core). */
function labelled(keys: readonly string[]): Labelled[] {
  return keys.map(fixtureLabel).sort((a, b) => a.label.localeCompare(b.label));
}

interface Group {
  label: string;
  provider: string | null;
  events: FixtureEvent[];
}

function daysOf(start: string, end: string): string[] {
  const days: string[] = [];
  for (let d = start; d <= end; d = addDays(d, 1)) days.push(d);
  return days;
}

/** Groups events by a key; `describe` names a group the first time it is seen. */
function groupBy(
  selected: readonly Flat[],
  keyOf: (f: Flat) => string,
  describe: (f: Flat) => Omit<Group, "events">,
): Map<string, Group> {
  const groups = new Map<string, Group>();
  for (const f of selected) {
    const key = keyOf(f);
    const group = groups.get(key) ?? { ...describe(f), events: [] };
    group.events.push(f.event);
    groups.set(key, group);
  }
  return groups;
}

/** One period's headline values, `null` where nothing was reported (like the core). */
function headline(period: readonly Flat[]) {
  const sums = measuresOf(period.map((f) => f.event));
  const act = activity(period);
  const read = period.reduce((n, f) => n + f.event.cacheRead, 0);
  const prompt = period.reduce(
    (n, f) => n + f.event.input + f.event.cacheRead + f.event.cacheWrite,
    0,
  );
  const reported = period.length > 0;
  return {
    tokens: reported ? sums.tokens : null,
    costNanoUsd: sums.costNanoUsd,
    requests: reported ? sums.requests : null,
    sessions: act.sessions,
    activeMs: act.activeMs,
    cacheHitRatio: prompt === 0 ? null : read / prompt,
  };
}

function kpis(selected: readonly Flat[], before: readonly Flat[] | null): Overview["kpis"] {
  const now = headline(selected);
  const then = before === null ? null : headline(before);
  return {
    tokens: kpi(now.tokens, then?.tokens ?? null, "Fixture: all prompt and output tokens."),
    costNanoUsd: {
      ...kpi(now.costNanoUsd, then?.costNanoUsd ?? null, "Fixture: API-equivalent cost."),
      accuracy: "estimated",
    },
    requests: kpi(now.requests, then?.requests ?? null, "Fixture: model API calls."),
    sessions: kpi(now.sessions, then?.sessions ?? null, "Fixture: distinct sessions."),
    activeMs: {
      ...kpi(now.activeMs, then?.activeMs ?? null, "Fixture: capped request gaps."),
      accuracy: null,
    },
    cacheHitRatio: kpi(now.cacheHitRatio, null, "Fixture: cache reads over prompt tokens."),
  };
}

function composition(hasData: boolean): Overview["composition"] {
  return {
    exact: hasData ? 1 : 0,
    providerReported: 0,
    partial: 0,
    estimated: 0,
    unsupported: 0,
    weakest: hasData ? "exact" : null,
    pricingCoverage: hasData ? 1 : null,
  };
}

function daily(selected: readonly Flat[], scope: Scope): DailyPoint[] {
  const byDay = groupBy(
    selected,
    (f) => localDay(f.event.at, scope.timeZone),
    () => ({ label: "", provider: null }),
  );
  return daysOf(scope.start, scope.end).map((date) => ({
    date,
    values: [measuresOf(byDay.get(date)?.events ?? [])],
  }));
}

export function overview(flat: readonly Flat[], scope: Scope): Overview {
  const selected = select(flat, scope, scope.start, scope.end);
  const prev = previousRange(scope.start, scope.end);
  const before = scope.comparison === "previous" ? select(flat, scope, prev.start, prev.end) : null;
  const tokens = measuresOf(selected.map((f) => f.event)).tokens;
  const byModel = groupBy(
    selected,
    (f) => f.event.model,
    (f) => ({ label: fixtureLabel(f.event.model).label, provider: f.event.provider }),
  );
  const byProject = groupBy(
    selected,
    (f) => String(f.session.projectId),
    (f) => ({ label: FIXTURE_PROJECTS[f.session.projectId - 1] ?? "No project", provider: null }),
  );
  return {
    kpis: kpis(selected, before),
    composition: composition(selected.length > 0),
    apps: labelled(["claude-code"]),
    providers: labelled([...new Set(selected.map((f) => f.event.provider))]),
    daily: daily(selected, scope),
    models: rank(byModel, 8, tokens),
    projects: rank(byProject, 30, tokens),
    calendar: calendar(flat, scope),
    hours: hours(selected, scope.timeZone),
  };
}

function calendar(flat: readonly Flat[], scope: Scope): CalendarDay[] {
  const start = addDays(scope.end, -364);
  const totals = new Map<string, number>();
  for (const { event } of select(flat, scope, start, scope.end)) {
    const day = localDay(event.at, scope.timeZone);
    totals.set(day, (totals.get(day) ?? 0) + tokensOf(event));
  }
  return daysOf(start, scope.end).map((date) => ({ date, tokens: totals.get(date) ?? 0 }));
}

function hours(selected: readonly Flat[], timeZone: string): HourCell[] {
  const grid = Array.from({ length: 7 }, () => Array<number>(24).fill(0));
  for (const { event } of selected) {
    const { weekday, hour } = localHourWeekday(event.at, timeZone);
    const row = grid[weekday];
    if (row !== undefined) row[hour] = (row[hour] ?? 0) + tokensOf(event);
  }
  return grid.flatMap((row, weekday) => row.map((tokens, hour) => ({ weekday, hour, tokens })));
}

type Row = SessionList["rows"][number];

const SORT_VALUE: Readonly<Record<SessionsRequest["sort"], (r: Row) => number>> = {
  lastActive: (r) => r.endedAt,
  tokens: (r) => r.measures.tokens,
  cost: (r) => r.measures.costNanoUsd,
  events: (r) => r.events,
};

export function sessions(flat: readonly Flat[], request: SessionsRequest): SessionList {
  const selected = select(flat, request.scope, request.scope.start, request.scope.end);
  const bySession = new Map<number, { session: Flat["session"]; events: FixtureEvent[] }>();
  for (const { session, event } of selected) {
    const entry = bySession.get(session.id) ?? { session, events: [] };
    entry.events.push(event);
    bySession.set(session.id, entry);
  }
  const rows = [...bySession.values()].map((s) =>
    sessionRow(s.session, s.events, FIXTURE_PROJECTS),
  );
  const value = SORT_VALUE[request.sort];
  rows.sort((a, b) => value(b) - value(a) || b.id - a.id);
  const from = request.page * request.pageSize;
  return { total: rows.length, rows: rows.slice(from, from + request.pageSize) };
}
