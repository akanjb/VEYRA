/**
 * Aggregations over the synthetic dataset, shaped like the core's responses. They exist
 * only to feed the UI realistic data in development and tests; product numbers always
 * come from the Rust analytics engine.
 */
import type { Kpi, Measures, RankRow, Scope, SessionRow } from "@veyra/contracts";

import { addDays, daysInclusive } from "../../filters/dates";
import { fixtureLabel, type FixtureEvent, type FixtureSession } from "./dataset";

export const IDLE_MS = 300_000;

export interface Flat {
  session: FixtureSession;
  event: FixtureEvent;
}

export function localDay(at: number, timeZone: string): string {
  return new Intl.DateTimeFormat("en-CA", {
    timeZone,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(at);
}

export function localHourWeekday(at: number, timeZone: string): { weekday: number; hour: number } {
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone,
    weekday: "short",
    hour: "numeric",
    hourCycle: "h23",
  }).formatToParts(at);
  const weekday = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].indexOf(
    parts.find((p) => p.type === "weekday")?.value ?? "Mon",
  );
  return { weekday, hour: Number(parts.find((p) => p.type === "hour")?.value ?? 0) };
}

export const tokensOf = (e: FixtureEvent): number =>
  e.input + e.cacheRead + e.cacheWrite + e.output;

export function measuresOf(events: readonly FixtureEvent[]): Measures {
  return events.reduce<Measures>(
    (m, e) => ({
      tokens: m.tokens + tokensOf(e),
      costNanoUsd: m.costNanoUsd + e.costNanoUsd,
      requests: m.requests + 1,
    }),
    { tokens: 0, costNanoUsd: 0, requests: 0 },
  );
}

/** Events in `[start, end]` (local days) matching the scope's filters. */
export function select(flat: readonly Flat[], scope: Scope, start: string, end: string): Flat[] {
  const f = scope.filters;
  const allowed = (list: string[], value: string) => list.length === 0 || list.includes(value);
  return flat.filter(({ session, event }) => {
    const day = localDay(event.at, scope.timeZone);
    return (
      day >= start &&
      day <= end &&
      allowed(f.models, event.model) &&
      allowed(f.providers, event.provider) &&
      allowed(f.apps, session.app) &&
      allowed(f.interfaces, session.interface) &&
      allowed(f.projects, String(session.projectId))
    );
  });
}

/** The comparison range: the same number of days immediately before. */
export function previousRange(start: string, end: string): { start: string; end: string } {
  const length = daysInclusive(start, end);
  return { start: addDays(start, -length), end: addDays(end, -length) };
}

/** Distinct sessions and their union of capped gaps (the PRD active-time definition). */
export function activity(flat: readonly Flat[]): { sessions: number; activeMs: number } {
  const bySession = new Map<number, number[]>();
  for (const { session, event } of flat) {
    const times = bySession.get(session.id) ?? [];
    times.push(event.at);
    bySession.set(session.id, times);
  }
  const intervals: [number, number][] = [];
  for (const times of bySession.values()) {
    times.sort((a, b) => a - b);
    for (let i = 1; i < times.length; i += 1) {
      const from = times[i - 1] ?? 0;
      intervals.push([from, from + Math.min((times[i] ?? 0) - from, IDLE_MS)]);
    }
  }
  intervals.sort((a, b) => a[0] - b[0]);
  let active = 0;
  let current: [number, number] | null = null;
  for (const interval of intervals) {
    if (current !== null && interval[0] <= current[1])
      current[1] = Math.max(current[1], interval[1]);
    else {
      if (current !== null) active += current[1] - current[0];
      current = [...interval];
    }
  }
  if (current !== null) active += current[1] - current[0];
  return { sessions: bySession.size, activeMs: active };
}

/** A KPI; the change is undefined when either value is unknown or the previous is zero. */
export function kpi(value: number | null, previous: number | null, definition: string): Kpi {
  return {
    value,
    previous,
    change:
      value === null || previous === null || previous === 0 ? null : (value - previous) / previous,
    accuracy: value === null ? null : "exact",
    definition,
  };
}

/** Top `n` groups by tokens plus an exact "Other". */
export function rank(
  groups: Map<string, { label: string; provider: string | null; events: FixtureEvent[] }>,
  n: number,
  scopeTokens: number,
): RankRow[] {
  const rows = [...groups.entries()]
    .map(([key, g]) => ({ key, ...g, measures: measuresOf(g.events) }))
    .sort((a, b) => b.measures.tokens - a.measures.tokens);
  const share = (m: Measures) => (scopeTokens === 0 ? 0 : m.tokens / scopeTokens);
  const top: RankRow[] = rows.slice(0, n).map((r) => ({
    key: r.key,
    label: r.label,
    provider: r.provider,
    measures: r.measures,
    share: share(r.measures),
    groupCount: 1,
    isOther: false,
  }));
  const rest = rows.slice(n);
  if (rest.length > 0) {
    const measures = measuresOf(rest.flatMap((r) => r.events));
    top.push({
      key: null,
      label: "Other",
      provider: null,
      measures,
      share: share(measures),
      groupCount: rest.length,
      isOther: true,
    });
  }
  return top;
}

export function sessionRow(
  session: FixtureSession,
  events: readonly FixtureEvent[],
  projects: readonly string[],
): SessionRow {
  const times = events.map((e) => e.at).sort((a, b) => a - b);
  const flat = events.map((event) => ({ session, event }));
  return {
    id: session.id,
    key: session.key,
    app: session.app,
    project: projects[session.projectId - 1] ?? null,
    models: [...new Set(events.map((e) => e.model))].sort().map(fixtureLabel),
    isSubagent: session.parentId !== null,
    parentId: session.parentId,
    startedAt: times[0] ?? 0,
    endedAt: times.at(-1) ?? 0,
    activeMs: activity(flat).activeMs,
    events: events.length,
    measures: measuresOf(events),
    unpricedRequests: 0,
    costAssumptions: false,
    accuracy: "exact",
  };
}

/** Events grouped by session, in the order given. */
export function bySession(flat: readonly Flat[]): Map<number, Flat[]> {
  const sessions = new Map<number, Flat[]>();
  for (const f of flat) {
    const list = sessions.get(f.session.id) ?? [];
    list.push(f);
    sessions.set(f.session.id, list);
  }
  return sessions;
}
