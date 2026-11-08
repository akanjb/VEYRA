import type { Overview } from "@veyra/contracts";

import {
  CalendarHeatmap,
  HourHeatmap,
  HourTable,
  RankTable,
  Ranking,
  StackedTrend,
  Treemap,
  TrendTable,
  useChartTheme,
} from "@/shared/charts";
import type { Measure } from "@/shared/filters";
import { MEASURE, fmt } from "@/shared/format";
import { ChartCard } from "@/shared/ui";

import { calendarDays, hourCells, modelItems, projectItems, trendInput } from "./overview-model";

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

export function CalendarCard({
  overview,
  onSelectDate,
}: {
  overview: Overview;
  onSelectDate: (date: string) => void;
}) {
  const days = calendarDays(overview);
  return (
    <ChartCard
      title="Activity calendar"
      definition="Tokens per local day over the year ending on the last day of the period. Click a day to narrow the period to it."
      accuracy={overview.composition.weakest}
      chart={
        <CalendarHeatmap
          days={days}
          format={fmt.compact}
          label="Tokens per day over the last year"
          onSelectDate={onSelectDate}
        />
      }
      table={
        <RankTable
          items={days
            .filter((d) => d.value > 0)
            .reverse()
            .map((d) => ({
              key: d.date,
              label: fmt.day(d.date),
              value: d.value,
              share: 0,
              color: "",
            }))}
          format={fmt.integer}
          caption="Tokens per day"
          header="Day"
        />
      }
    />
  );
}

export function HoursCard({ overview }: { overview: Overview }) {
  const cells = hourCells(overview);
  return (
    <ChartCard
      title="Hours of the week"
      definition="Tokens by local weekday and hour over the selected period."
      accuracy={overview.composition.weakest}
      chart={<HourHeatmap cells={cells} format={fmt.compact} label="Tokens by weekday and hour" />}
      table={<HourTable cells={cells} format={fmt.integer} caption="Tokens by weekday and hour" />}
    />
  );
}
