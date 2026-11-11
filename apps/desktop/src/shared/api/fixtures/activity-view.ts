/**
 * The Activity response built from the synthetic dataset, shaped like the core's. Only
 * for development and UI tests: product numbers always come from the Rust engine.
 */
import type { Activity, LengthBin, LongSession, Scope } from "@veyra/contracts";

import {
  IDLE_MS,
  activity as activityOf,
  kpi,
  localDay,
  previousRange,
  select,
  bySession,
  type Flat,
} from "./aggregate";
import { FIXTURE_PROJECTS, fixtureLabel } from "./dataset";
import { calendar, daysOf, hours } from "./views";

const BOUNDS_MS = [0, 60_000, 300_000, 900_000, 1_800_000, 3_600_000, 7_200_000, 14_400_000];

interface SessionSpan {
  flat: Flat[];
  first: number;
  last: number;
  activeMs: number;
}

function spans(selected: readonly Flat[]): SessionSpan[] {
  return [...bySession(selected).values()].map((flat) => {
    const times = flat.map((f) => f.event.at).sort((a, b) => a - b);
    const activeMs = times
      .slice(1)
      .reduce((n, t, i) => n + Math.min(t - (times[i] ?? t), IDLE_MS), 0);
    return { flat, first: times[0] ?? 0, last: times.at(-1) ?? 0, activeMs };
  });
}

function lengthBins(sessions: readonly SessionSpan[]): LengthBin[] {
  const bin = (ms: number) => BOUNDS_MS.filter((b) => b <= ms).length - 1;
  return BOUNDS_MS.map((lower, i) => ({
    lowerMs: lower,
    upperMs: BOUNDS_MS[i + 1] ?? null,
    byActive: sessions.filter((s) => bin(s.activeMs) === i).length,
    byWall: sessions.filter((s) => bin(s.last - s.first) === i).length,
  }));
}

function longest(sessions: readonly SessionSpan[]): LongSession[] {
  return [...sessions]
    .sort((a, b) => b.last - b.first - (a.last - a.first))
    .slice(0, 10)
    .map((s) => {
      const session = s.flat[0]?.session;
      return {
        id: session?.id ?? 0,
        app: "Claude Code",
        project: FIXTURE_PROJECTS[(session?.projectId ?? 1) - 1] ?? null,
        models: [...new Set(s.flat.map((f) => f.event.model))].map(fixtureLabel),
        startedAt: s.first,
        endedAt: s.last,
        activeMs: s.activeMs,
        wallMs: s.last - s.first,
      };
    });
}

/** Wall time as the sum of session spans (the fixture does not merge overlaps). */
const wallOf = (sessions: readonly SessionSpan[]) =>
  sessions.reduce((n, s) => n + (s.last - s.first), 0);

/** Each event's local day, computed once (formatting a date is the expensive part). */
function dayIndex(selected: readonly Flat[], timeZone: string) {
  const days = new Map<number, string>();
  const dayOf = (f: Flat) => {
    const known = days.get(f.event.at);
    if (known !== undefined) return known;
    const day = localDay(f.event.at, timeZone);
    days.set(f.event.at, day);
    return day;
  };
  const byDay = new Map<string, Flat[]>();
  for (const f of selected) {
    const list = byDay.get(dayOf(f)) ?? [];
    list.push(f);
    byDay.set(dayOf(f), list);
  }
  return { dayOf, byDay };
}

function kpis(
  selected: readonly Flat[],
  before: readonly Flat[] | null,
  dayOf: (f: Flat) => string,
): Activity["kpis"] {
  const tile = (value: number, previous: number | null, definition: string) => ({
    ...kpi(value, previous, definition),
    accuracy: null,
  });
  const now = activityOf(selected);
  const then = before === null ? null : activityOf(before);
  const activeDays = (period: readonly Flat[]) => new Set(period.map(dayOf)).size;
  return {
    activeMs: tile(now.activeMs, then?.activeMs ?? null, "Fixture: capped gaps."),
    wallMs: tile(
      wallOf(spans(selected)),
      before === null ? null : wallOf(spans(before)),
      "Fixture: first to last request.",
    ),
    sessions: tile(now.sessions, then?.sessions ?? null, "Fixture: distinct sessions."),
    activeDays: tile(
      activeDays(selected),
      before === null ? null : activeDays(before),
      "Fixture: days with requests.",
    ),
  };
}

export function activity(flat: readonly Flat[], scope: Scope): Activity {
  const selected = select(flat, scope, scope.start, scope.end);
  const prev = previousRange(scope.start, scope.end);
  const before = scope.comparison === "previous" ? select(flat, scope, prev.start, prev.end) : null;
  const { dayOf, byDay } = dayIndex(selected, scope.timeZone);
  const sessions = spans(selected);
  return {
    kpis: kpis(selected, before, dayOf),
    calendar: calendar(flat, scope),
    hours: hours(selected, scope.timeZone),
    daily: daysOf(scope.start, scope.end).map((date) => {
      const onDay = byDay.get(date) ?? [];
      const day = activityOf(onDay);
      return { date, sessions: day.sessions, activeMs: day.activeMs, wallMs: wallOf(spans(onDay)) };
    }),
    sessionLengths: lengthBins(sessions),
    longest: longest(sessions),
    idleThresholdMs: IDLE_MS,
    activeDefinition: "Fixture: gaps between requests capped at 5 minutes.",
    wallDefinition: "Fixture: from the first request to the last.",
  };
}
