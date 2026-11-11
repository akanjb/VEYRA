import type { OverviewKpis } from "@veyra/contracts";

import type { Measure } from "../filters";
import { MEASURE, fmt } from "../format";
import { KpiGrid, KpiTile } from "../ui";

import { dailyTotals, type UsageSummary } from "./summary-model";

type KpiKey = keyof OverviewKpis;

/** Each headline tile: its label and formatting. Measures reuse the measure registry. */
const TILES: Readonly<
  Record<
    KpiKey,
    {
      label: string;
      format: (v: number | null) => string;
      exact?: (v: number | null) => string;
      derived?: boolean;
    }
  >
> = {
  tokens: {
    label: MEASURE.tokens.label,
    format: MEASURE.tokens.format,
    exact: MEASURE.tokens.exact,
  },
  costNanoUsd: {
    label: MEASURE.cost.label,
    format: MEASURE.cost.format,
    exact: MEASURE.cost.exact,
  },
  requests: {
    label: MEASURE.requests.label,
    format: MEASURE.requests.format,
    exact: MEASURE.requests.exact,
  },
  sessions: { label: "Sessions", format: (v) => fmt.integer(v) },
  activeMs: { label: "Active time", format: (v) => fmt.duration(v), derived: true },
  cacheHitRatio: { label: "Cache hit ratio", format: (v) => fmt.percent(v) },
};

/** The KPI behind each chart measure. */
const HERO: Readonly<Record<Measure, KpiKey>> = {
  tokens: "tokens",
  cost: "costNanoUsd",
  requests: "requests",
};

const ORDER: readonly KpiKey[] = [
  "tokens",
  "costNanoUsd",
  "requests",
  "sessions",
  "activeMs",
  "cacheHitRatio",
];

/** One hero number for the selected measure, then the supporting tiles (§3). */
export function KpiStrip({ summary, measure }: { summary: UsageSummary; measure: Measure }) {
  const hero = HERO[measure];
  const tile = (key: KpiKey, isHero: boolean) => {
    const meta = TILES[key];
    return (
      <KpiTile
        key={key}
        label={meta.label}
        kpi={summary.kpis[key]}
        format={meta.format}
        {...(meta.exact === undefined ? {} : { exact: meta.exact })}
        {...(meta.derived === true ? { accuracy: "derived" as const } : {})}
        hero={isHero}
        {...(isHero ? { trend: dailyTotals(summary, measure) } : {})}
      />
    );
  };
  return (
    <KpiGrid>
      {tile(hero, true)}
      {ORDER.filter((k) => k !== hero).map((k) => tile(k, false))}
    </KpiGrid>
  );
}
