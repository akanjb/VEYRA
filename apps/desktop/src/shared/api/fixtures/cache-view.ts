/**
 * The Cache response built from the synthetic dataset, shaped like the core's. Only for
 * development and UI tests: product numbers always come from the Rust analytics engine.
 */
import type { Cache, CacheSession, RatioRow, Scope, TtlRow } from "@veyra/contracts";

import { kpi, localDay, previousRange, select, type Flat } from "./aggregate";
import { FIXTURE_PROJECTS, fixtureLabel, type FixtureEvent } from "./dataset";
import { daysOf, groupBy } from "./views";

/** Fixture prices per token, in nano-USD: input, cache read and 5-minute cache write. */
const INPUT_NANO = 3_000;
const READ_NANO = 300;
const WRITE_NANO = 3_750;

interface Totals {
  input: number;
  read: number;
  write: number;
}

function totals(events: readonly FixtureEvent[]): Totals {
  return events.reduce<Totals>(
    (t, e) => ({
      input: t.input + e.input + e.cacheRead + e.cacheWrite,
      read: t.read + e.cacheRead,
      write: t.write + e.cacheWrite,
    }),
    { input: 0, read: 0, write: 0 },
  );
}

const saving = (t: Totals) => t.read * (INPUT_NANO - READ_NANO);
const overhead = (t: Totals) => t.write * (WRITE_NANO - INPUT_NANO);
const ratio = (part: number, whole: number) => (whole === 0 ? null : part / whole);

function kpis(now: Totals, then: Totals | null, reported: boolean): Cache["kpis"] {
  const value = (pick: (t: Totals) => number | null) => (reported ? pick(now) : null);
  const before = (pick: (t: Totals) => number | null) => (then === null ? null : pick(then));
  const tile = (pick: (t: Totals) => number | null, definition: string, money: boolean) => ({
    ...kpi(value(pick), before(pick), definition),
    ...(money ? { accuracy: "estimated" as const } : {}),
  });
  return {
    netBenefitNanoUsd: tile(
      (t) => saving(t) - overhead(t),
      "Fixture: saving minus overhead.",
      true,
    ),
    grossSavingNanoUsd: tile(saving, "Fixture: input-rate cost avoided by reads.", true),
    writeOverheadNanoUsd: tile(overhead, "Fixture: extra cost of writes.", true),
    hitRatio: tile((t) => ratio(t.read, t.input), "Fixture: reads over input.", false),
    reuseRatio: tile((t) => ratio(t.read, t.write), "Fixture: reads per written token.", false),
  };
}

function ratioRows(
  selected: readonly Flat[],
  keyOf: (f: Flat) => { key: string; label: string; provider: string | null },
): RatioRow[] {
  const groups = groupBy(
    selected,
    (f) => keyOf(f).key,
    (f) => keyOf(f),
  );
  return [...groups.entries()]
    .map(([key, g]) => ({ key, g, t: totals(g.events) }))
    .sort((a, b) => b.t.input - a.t.input)
    .map(({ key, g, t }) => ({
      key,
      label: g.label,
      provider: g.provider,
      hitRatio: ratio(t.read, t.input),
      inputTokens: t.input,
      cacheRead: t.read,
      isOther: false,
    }));
}

function ttlRows(selected: readonly Flat[]): TtlRow[] {
  const groups = groupBy(
    selected,
    (f) => f.event.model,
    (f) => ({ label: fixtureLabel(f.event.model).label, provider: f.event.provider }),
  );
  return [...groups.entries()]
    .map(([key, g]) => ({ key, g, t: totals(g.events) }))
    .filter(({ t }) => t.write > 0)
    .sort((a, b) => b.t.write - a.t.write)
    .map(({ key, g, t }) => ({
      key,
      label: g.label,
      provider: g.provider,
      short: t.write,
      long: 0,
      unspecified: 0,
      reads: t.read,
    }));
}

function losses(selected: readonly Flat[]): CacheSession[] {
  const bySession = new Map<number, Flat[]>();
  for (const f of selected)
    bySession.set(f.session.id, [...(bySession.get(f.session.id) ?? []), f]);
  return [...bySession.values()]
    .map((flat) => {
      const session = flat[0]?.session;
      const t = totals(flat.map((f) => f.event));
      return { flat, session, t, net: saving(t) - overhead(t) };
    })
    .filter((s) => s.session !== undefined && s.net < 0)
    .sort((a, b) => a.net - b.net)
    .slice(0, 10)
    .map(({ flat, session, t, net }) => ({
      id: session?.id ?? 0,
      app: "Claude Code",
      project: FIXTURE_PROJECTS[(session?.projectId ?? 1) - 1] ?? null,
      models: [...new Set(flat.map((f) => f.event.model))].map(fixtureLabel),
      endedAt: Math.max(...flat.map((f) => f.event.at)),
      cacheWrite: t.write,
      cacheRead: t.read,
      reuseRatio: ratio(t.read, t.write),
      netBenefitNanoUsd: net,
    }));
}

export function cache(flat: readonly Flat[], scope: Scope): Cache {
  const selected = select(flat, scope, scope.start, scope.end);
  const prev = previousRange(scope.start, scope.end);
  const before = scope.comparison === "previous" ? select(flat, scope, prev.start, prev.end) : null;
  const byDay = groupBy(
    selected,
    (f) => localDay(f.event.at, scope.timeZone),
    () => ({ label: "", provider: null }),
  );
  const reported = selected.length > 0;
  return {
    kpis: kpis(
      totals(selected.map((f) => f.event)),
      before === null || before.length === 0 ? null : totals(before.map((f) => f.event)),
      reported,
    ),
    accuracy: reported ? "exact" : null,
    pricingCoverage: reported ? 1 : null,
    coverageDefinition: "Fixture: every model is priced.",
    daily: daysOf(scope.start, scope.end).map((date) => {
      const t = totals(byDay.get(date)?.events ?? []);
      return { date, reads: t.read, writes: t.write };
    }),
    byModel: ratioRows(selected, (f) => ({
      key: f.event.model,
      label: fixtureLabel(f.event.model).label,
      provider: f.event.provider,
    })),
    byProject: ratioRows(selected, (f) => ({
      key: String(f.session.projectId),
      label: FIXTURE_PROJECTS[f.session.projectId - 1] ?? "No project",
      provider: null,
    })),
    writesByModel: ttlRows(selected),
    inefficient: losses(selected),
    inefficientDefinition: "Fixture: sessions whose writes cost more than their reads saved.",
  };
}
