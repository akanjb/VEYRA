import { RankTable, Ranking, StackedTrend, TrendTable, useChartTheme } from "../charts";
import type { Measure } from "../filters";
import { MEASURE } from "../format";
import { ChartCard } from "../ui";

import { modelItems, trendInput, type UsageSummary } from "./summary-model";

interface CardProps {
  summary: UsageSummary;
  measure: Measure;
}

export function TrendCard({
  summary,
  measure,
  onSelectDate,
}: CardProps & { onSelectDate: (date: string) => void }) {
  const theme = useChartTheme();
  const { dates, series } = trendInput(summary, measure, theme);
  const meta = MEASURE[measure];
  return (
    <ChartCard
      title={`Daily ${meta.label.toLowerCase()} by application`}
      definition="Each bar is one local day; click a day to narrow the period to it."
      accuracy={meta.estimated ? "estimated" : summary.composition.weakest}
      chart={
        <StackedTrend
          dates={dates}
          series={series}
          format={meta.format}
          label={`Daily ${meta.label} by application`}
          onSelectDate={onSelectDate}
        />
      }
      table={
        <TrendTable
          dates={dates}
          series={series}
          format={meta.exact}
          caption={`Daily ${meta.label} by application`}
        />
      }
    />
  );
}

export function ModelMixCard({
  summary,
  measure,
  onSelect,
}: CardProps & { onSelect?: (model: string) => void }) {
  const theme = useChartTheme();
  const items = modelItems(summary, measure, theme);
  const meta = MEASURE[measure];
  return (
    <ChartCard
      title="Model mix"
      definition={`Top models by the selected measure, plus everything else merged exactly.${onSelect === undefined ? "" : " Click a model to filter by it."}`}
      accuracy={meta.estimated ? "estimated" : summary.composition.weakest}
      chart={
        <Ranking
          items={items}
          format={meta.format}
          label="Model mix"
          {...(onSelect === undefined ? {} : { onSelect })}
        />
      }
      table={<RankTable items={items} format={meta.exact} caption="Model mix" header="Model" />}
    />
  );
}
