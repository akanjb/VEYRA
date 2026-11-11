import type { Overview } from "@veyra/contracts";

import {
  RankTable,
  Ranking,
  StackedTrend,
  Treemap,
  TrendTable,
  useChartTheme,
} from "@/shared/charts";
import type { Measure } from "@/shared/filters";
import { MEASURE } from "@/shared/format";
import { ChartCard } from "@/shared/ui";

import { modelItems, projectItems, trendInput } from "./overview-model";

interface CardProps {
  overview: Overview;
  measure: Measure;
}

export function TrendCard({
  overview,
  measure,
  onSelectDate,
}: CardProps & { onSelectDate: (date: string) => void }) {
  const theme = useChartTheme();
  const { dates, series } = trendInput(overview, measure, theme);
  const meta = MEASURE[measure];
  return (
    <ChartCard
      title={`Daily ${meta.label.toLowerCase()} by application`}
      definition="Each bar is one local day; click a day to narrow the period to it."
      accuracy={meta.estimated ? "estimated" : overview.composition.weakest}
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
  overview,
  measure,
  onSelect,
}: CardProps & { onSelect: (model: string) => void }) {
  const theme = useChartTheme();
  const items = modelItems(overview, measure, theme);
  const meta = MEASURE[measure];
  return (
    <ChartCard
      title="Model mix"
      definition="Top models by the selected measure, plus everything else merged exactly. Click a model to filter by it."
      accuracy={meta.estimated ? "estimated" : overview.composition.weakest}
      chart={<Ranking items={items} format={meta.format} label="Model mix" onSelect={onSelect} />}
      table={<RankTable items={items} format={meta.exact} caption="Model mix" header="Model" />}
    />
  );
}

export function ProjectsCard({
  overview,
  measure,
  onSelect,
}: CardProps & { onSelect: (project: string) => void }) {
  const theme = useChartTheme();
  const items = projectItems(overview, measure, theme);
  const meta = MEASURE[measure];
  return (
    <ChartCard
      title="Projects"
      definition="Usage per project (working directory), top 30 plus everything else. Click a project to filter by it."
      accuracy={meta.estimated ? "estimated" : overview.composition.weakest}
      chart={
        <Treemap items={items} format={meta.format} label="Usage by project" onSelect={onSelect} />
      }
      table={
        <RankTable items={items} format={meta.exact} caption="Usage by project" header="Project" />
      }
    />
  );
}
