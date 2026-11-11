import type { Cache, CacheKpis, CacheSession, RatioRow } from "@veyra/contracts";

import {
  DataTable,
  GroupedBars,
  GroupedTable,
  MirroredTable,
  MirroredTrend,
  Ranking,
  useChartTheme,
} from "@/shared/charts";
import { MEASURE, MISSING, fmt } from "@/shared/format";
import { useOpenSession } from "@/shared/links";
import { Card, ChartCard, EmptyState, KpiGrid, KpiTile } from "@/shared/ui";

import { flowInput, ratioItems, ttlInput } from "./cache-model";

const money = MEASURE.cost;
const tokens = MEASURE.tokens;
const percent = (v: number | null) => fmt.percent(v);

const TILES: readonly {
  key: keyof CacheKpis;
  label: string;
  format: (v: number | null) => string;
  exact?: (v: number | null) => string;
}[] = [
  {
    key: "netBenefitNanoUsd",
    label: "Net cache benefit",
    format: money.format,
    exact: money.exact,
  },
  { key: "grossSavingNanoUsd", label: "Gross saving", format: money.format, exact: money.exact },
  {
    key: "writeOverheadNanoUsd",
    label: "Write overhead",
    format: money.format,
    exact: money.exact,
  },
  { key: "hitRatio", label: "Cache hit ratio", format: percent },
  { key: "reuseRatio", label: "Cache reuse", format: (v) => fmt.times(v) },
];

export function CacheKpiStrip({ cache, trend }: { cache: Cache; trend: readonly number[] }) {
  return (
    <KpiGrid>
      {TILES.map((tile, i) => (
        <KpiTile
          key={tile.key}
          label={tile.label}
          kpi={cache.kpis[tile.key]}
          format={tile.format}
          {...(tile.exact === undefined ? {} : { exact: tile.exact })}
          hero={i === 0}
          {...(i === 0 ? { trend } : {})}
        />
      ))}
    </KpiGrid>
  );
}

/** States how much of the usage the money figures cover, when not all of it. */
export function CoverageNote({ cache }: { cache: Cache }) {
  const coverage = cache.pricingCoverage;
  if (coverage === null || coverage >= 1) return null;
  return (
    <p className="text-xs text-muted" title={cache.coverageDefinition}>
      Savings and overheads cover {fmt.percent(coverage)} of tokens; the rest are on models without
      published cache prices.
    </p>
  );
}

export function FlowCard({ cache }: { cache: Cache }) {
  const theme = useChartTheme();
  const { dates, reads, writes } = flowInput(cache, theme);
  return (
    <ChartCard
      title="Cache reads and writes"
      definition="Tokens read from the cache (above the axis) and written to it (below), per local day."
      accuracy={cache.accuracy}
      chart={
        <MirroredTrend
          dates={dates}
          above={reads}
          below={writes}
          format={tokens.format}
          label="Cache reads and writes per day"
        />
      }
      table={
        <MirroredTable
          dates={dates}
          above={reads}
          below={writes}
          format={tokens.exact}
          caption="Cache reads and writes per day"
        />
      }
    />
  );
}

function RatioTable({
  rows,
  caption,
  header,
}: {
  rows: readonly RatioRow[];
  caption: string;
  header: string;
}) {
  return (
    <DataTable
      caption={caption}
      rows={rows}
      rowKey={(r, i) => `${r.key ?? "none"}-${String(i)}`}
      columns={[
        { header, cell: (r) => <span className="break-all">{r.label}</span> },
        { header: "Hit ratio", numeric: true, cell: (r) => fmt.percent(r.hitRatio) },
        { header: "Cache reads", numeric: true, cell: (r) => fmt.integer(r.cacheRead) },
        { header: "Input tokens", numeric: true, cell: (r) => fmt.integer(r.inputTokens) },
      ]}
    />
  );
}

export function HitRatioCard({
  cache,
  by,
  onSelect,
}: {
  cache: Cache;
  by: "model" | "project";
  onSelect: (key: string) => void;
}) {
  const theme = useChartTheme();
  const rows = by === "model" ? cache.byModel : cache.byProject;
  const items = ratioItems(rows, by === "model" ? "model" : "category", theme);
  const title = `Hit ratio by ${by}`;
  return (
    <ChartCard
      title={title}
      definition={`Cache reads as a share of all input tokens, for the ${by}s with the most input. Click a bar to filter by it.`}
      accuracy={cache.accuracy}
      chart={
        <Ranking
          items={items}
          format={percent}
          label={title}
          onSelect={onSelect}
          barLabel="value"
        />
      }
      table={
        <RatioTable rows={rows} caption={title} header={by === "model" ? "Model" : "Project"} />
      }
    />
  );
}

export function TtlCard({ cache }: { cache: Cache }) {
  const theme = useChartTheme();
  const { categories, series } = ttlInput(cache, theme);
  return (
    <ChartCard
      title="Cache writes by lifetime"
      definition="Tokens written with a 5-minute or 1-hour lifetime, next to the tokens read, for the models that write the most. Sources do not record which write a read used, so reads are not split by lifetime."
      accuracy={cache.accuracy}
      chart={
        <GroupedBars
          categories={categories}
          series={series}
          format={tokens.format}
          label="Cache writes by lifetime and cache reads, per model"
        />
      }
      table={
        <GroupedTable
          categories={categories}
          series={series}
          format={tokens.exact}
          caption="Cache writes by lifetime and cache reads, per model"
          header="Model"
        />
      }
    />
  );
}

export function LossesCard({ cache }: { cache: Cache }) {
  const open = useOpenSession();
  const rows = cache.inefficient;
  return (
    <Card
      title="Sessions where caching cost more than it saved"
      definition={cache.inefficientDefinition}
      accuracy="estimated"
    >
      {rows.length === 0 ? (
        <EmptyState title="No session lost money on caching in this period" />
      ) : (
        <DataTable<CacheSession>
          caption="Sessions where caching cost more than it saved"
          rows={rows}
          rowKey={(r) => String(r.id)}
          onRowClick={(r) => {
            open(r.id);
          }}
          columns={[
            { header: "Ended", cell: (r) => fmt.dateTime(r.endedAt) },
            {
              header: "Project",
              cell: (r) => (r.project === null ? MISSING : fmt.basename(r.project)),
            },
            { header: "Models", cell: (r) => r.models.map((m) => m.label).join(", ") },
            { header: "Writes", numeric: true, cell: (r) => fmt.compact(r.cacheWrite) },
            { header: "Reads", numeric: true, cell: (r) => fmt.compact(r.cacheRead) },
            {
              header: "Reuse",
              numeric: true,
              cell: (r) => fmt.times(r.reuseRatio),
            },
            { header: "Net benefit", numeric: true, cell: (r) => money.exact(r.netBenefitNanoUsd) },
          ]}
        />
      )}
    </Card>
  );
}
