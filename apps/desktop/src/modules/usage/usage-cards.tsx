import type { BreakdownDimension, Usage, UsageKpis } from "@veyra/contracts";

import {
  BandTable,
  BandTrend,
  Histogram,
  HistogramTable,
  Pareto,
  ParetoTable,
  RankTable,
  Ranking,
  StackedTrend,
  TrendTable,
  useChartTheme,
} from "@/shared/charts";
import { MEASURE, fmt } from "@/shared/format";
import { ChartCard, KpiGrid, KpiTile, Segmented } from "@/shared/ui";

import {
  BREAKDOWN,
  bandPoints,
  breakdownItems,
  breakdownOf,
  compositionInput,
  histogramBins,
} from "./usage-model";

const tokens = MEASURE.tokens;

const TILES: readonly { key: keyof UsageKpis; label: string; derived?: boolean }[] = [
  { key: "tokens", label: "Tokens" },
  { key: "inputTokens", label: "Input tokens" },
  { key: "outputTokens", label: "Output tokens" },
  { key: "reasoningTokens", label: "Reasoning tokens" },
  { key: "requests", label: "Requests" },
  { key: "tokensPerRequest", label: "Tokens per request", derived: true },
];

export function UsageKpiStrip({ usage, trend }: { usage: Usage; trend: readonly number[] }) {
  return (
    <KpiGrid>
      {TILES.map((tile, i) => (
        <KpiTile
          key={tile.key}
          label={tile.label}
          kpi={usage.kpis[tile.key]}
          format={tokens.format}
          exact={tokens.exact}
          {...(tile.derived === true ? { accuracy: "derived" as const } : {})}
          hero={i === 0}
          {...(i === 0 ? { trend } : {})}
        />
      ))}
    </KpiGrid>
  );
}

export function CompositionCard({
  usage,
  onSelectDate,
}: {
  usage: Usage;
  onSelectDate: (date: string) => void;
}) {
  const theme = useChartTheme();
  const { dates, series, overlay } = compositionInput(usage, theme);
  return (
    <ChartCard
      title="Token composition"
      definition="Tokens per local day by kind: uncached input, cache reads, cache writes and output add up to the total. Reasoning is part of output and is drawn as a line where sources report it. Click a day to narrow the period to it."
      accuracy={usage.accuracy}
      chart={
        <StackedTrend
          dates={dates}
          series={series}
          overlay={overlay}
          format={tokens.format}
          label="Daily tokens by kind"
          onSelectDate={onSelectDate}
        />
      }
      table={
        <TrendTable
          dates={dates}
          series={series}
          overlay={overlay}
          format={tokens.exact}
          caption="Daily tokens by kind"
        />
      }
    />
  );
}

const DIMENSION_OPTIONS = (Object.keys(BREAKDOWN) as BreakdownDimension[]).map((value) => ({
  value,
  label: BREAKDOWN[value].label,
}));

export function DimensionTabs({
  value,
  onChange,
}: {
  value: BreakdownDimension;
  onChange: (value: BreakdownDimension) => void;
}) {
  return (
    <Segmented
      value={value}
      options={DIMENSION_OPTIONS}
      onChange={onChange}
      label="Break down by"
    />
  );
}

export function BreakdownCard({
  usage,
  dimension,
  onSelect,
}: {
  usage: Usage;
  dimension: BreakdownDimension;
  onSelect: ((key: string) => void) | undefined;
}) {
  const theme = useChartTheme();
  const breakdown = breakdownOf(usage, dimension);
  if (breakdown === undefined) return null;
  const items = breakdownItems(breakdown, theme);
  const meta = BREAKDOWN[dimension];
  return (
    <ChartCard
      title={`Tokens by ${meta.label.toLowerCase()}`}
      definition={`The largest ${meta.noun} by tokens, plus everything else merged exactly.${onSelect === undefined ? "" : ` Click a bar to filter by it.`}`}
      accuracy={usage.accuracy}
      chart={
        <Ranking
          items={items}
          format={tokens.format}
          label={`Tokens by ${meta.label.toLowerCase()}`}
          {...(onSelect === undefined ? {} : { onSelect })}
        />
      }
      table={
        <RankTable
          items={items}
          format={tokens.exact}
          caption={`Tokens by ${meta.label.toLowerCase()}`}
          header={meta.label}
        />
      }
    />
  );
}

export function ConcentrationCard({
  usage,
  dimension,
}: {
  usage: Usage;
  dimension: BreakdownDimension;
}) {
  const theme = useChartTheme();
  const breakdown = breakdownOf(usage, dimension);
  if (breakdown === undefined) return null;
  const items = breakdownItems(breakdown, theme);
  const meta = BREAKDOWN[dimension];
  const index = breakdown.concentration;
  return (
    <ChartCard
      title={`Concentration by ${meta.label.toLowerCase()}`}
      definition={`Bars are ${meta.noun} by tokens; the line is the running share of all tokens. ${usage.concentrationDefinition}`}
      accuracy={usage.accuracy}
      actions={
        index === null ? null : (
          <span className="text-xs text-muted" data-numeric>
            Index {fmt.percent(index)}
          </span>
        )
      }
      chart={
        <Pareto
          items={items}
          cumulative={breakdown.cumulativeShare}
          format={tokens.format}
          label={`Running share of tokens by ${meta.label.toLowerCase()}`}
        />
      }
      table={
        <ParetoTable
          items={items}
          cumulative={breakdown.cumulativeShare}
          format={tokens.exact}
          caption={`Running share of tokens by ${meta.label.toLowerCase()}`}
          header={meta.label}
        />
      }
    />
  );
}

export function RequestSizeCard({ usage }: { usage: Usage }) {
  const points = bandPoints(usage.perRequest);
  return (
    <ChartCard
      title="Tokens per request"
      definition={usage.requestDefinition}
      accuracy="estimated"
      chart={
        <BandTrend
          points={points}
          format={tokens.format}
          label="Tokens per request: median, 90th percentile and mean per day"
          counted="Requests"
        />
      }
      table={
        <BandTable
          points={points}
          format={tokens.exact}
          caption="Tokens per request per day"
          counted="Requests"
        />
      }
    />
  );
}

export function SessionSizeCard({ usage }: { usage: Usage }) {
  const points = bandPoints(usage.perSession);
  return (
    <ChartCard
      title="Tokens per session"
      definition={usage.sessionDefinition}
      accuracy={usage.accuracy}
      chart={
        <BandTrend
          points={points}
          format={tokens.format}
          label="Tokens per session: median, 90th percentile and mean per day"
          counted="Sessions"
        />
      }
      table={
        <BandTable
          points={points}
          format={tokens.exact}
          caption="Tokens per session per day"
          counted="Sessions"
        />
      }
    />
  );
}

export function RequestDistributionCard({ usage }: { usage: Usage }) {
  const bins = histogramBins(usage);
  return (
    <ChartCard
      title="Request sizes"
      definition="How many requests fall in each size range over the period. Each range is about 19% wider than the one before, so the size axis is logarithmic. Only sources that record individual requests are included."
      accuracy={usage.accuracy}
      chart={<Histogram bins={bins} label="Requests by size in tokens" counted="Requests" />}
      table={<HistogramTable bins={bins} caption="Requests by size in tokens" counted="Requests" />}
    />
  );
}
