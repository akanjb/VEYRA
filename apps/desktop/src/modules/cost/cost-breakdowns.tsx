/** The Cost page's breakdowns: by group, by token kind, effective rates and change. */
import type { BreakdownDimension, Cost } from "@veyra/contracts";

import {
  DotPlot,
  DotTable,
  RankTable,
  Ranking,
  ShareBars,
  ShareTable,
  WaterfallChart,
  WaterfallTable,
  useChartTheme,
} from "@/shared/charts";
import { MEASURE, signedCost } from "@/shared/format";
import { ChartCard, Segmented } from "@/shared/ui";

import {
  BREAKDOWN,
  breakdownItems,
  breakdownOf,
  compositionParts,
  compositionRows,
  rateItems,
  waterfallBars,
} from "./cost-model";

const cost = MEASURE.cost;

const DIMENSIONS = (Object.keys(BREAKDOWN) as BreakdownDimension[]).map((value) => ({
  value,
  label: BREAKDOWN[value]?.label ?? value,
}));

/** Tabs choosing the dimension a breakdown or waterfall is shown by. */
export function DimensionTabs({
  value,
  onChange,
  only,
}: {
  value: BreakdownDimension;
  onChange: (value: BreakdownDimension) => void;
  /** Restricts the tabs to these dimensions. */
  only?: readonly BreakdownDimension[];
}) {
  const options =
    only === undefined ? DIMENSIONS : DIMENSIONS.filter((o) => only.includes(o.value));
  return <Segmented value={value} options={options} onChange={onChange} label="Show by" />;
}

export function CostBreakdownCard({
  data,
  dimension,
  onSelect,
}: {
  data: Cost;
  dimension: BreakdownDimension;
  onSelect: (key: string) => void;
}) {
  const theme = useChartTheme();
  const breakdown = breakdownOf(data, dimension);
  const meta = BREAKDOWN[dimension];
  if (breakdown === undefined || meta === undefined) return null;
  const items = breakdownItems(breakdown, theme);
  const title = `Cost by ${meta.label.toLowerCase()}`;
  return (
    <ChartCard
      title={title}
      definition={`The ${meta.noun} with the highest API-equivalent cost, plus everything else merged exactly; shares are of the period's cost. Click a bar to filter by it.`}
      accuracy="estimated"
      chart={<Ranking items={items} format={cost.format} label={title} onSelect={onSelect} />}
      table={<RankTable items={items} format={cost.exact} caption={title} header={meta.label} />}
    />
  );
}

export function CompositionCard({ data }: { data: Cost }) {
  const theme = useChartTheme();
  const parts = compositionParts(theme);
  const rows = compositionRows(data);
  return (
    <ChartCard
      title="Cost by token kind"
      definition="How API-equivalent cost divides into uncached input, cache reads, cache writes and output: for all models, then for the most expensive ones."
      accuracy="estimated"
      chart={<ShareBars parts={parts} rows={rows} format={cost.exact} label="Cost by token kind" />}
      table={
        <ShareTable
          parts={parts}
          rows={rows}
          format={cost.exact}
          caption="Cost by token kind"
          header="Models"
        />
      }
    />
  );
}

export function RatesCard({ data }: { data: Cost }) {
  const theme = useChartTheme();
  const items = rateItems(data, theme);
  return (
    <ChartCard
      title="Effective cost per 1M output tokens"
      definition="API-equivalent cost divided by output tokens, per model: an effective rate that includes input and cache, so models compare on what they actually cost for the work done. Highest first."
      accuracy="estimated"
      chart={
        <DotPlot
          items={items}
          format={cost.format}
          label="Effective cost per million output tokens by model"
        />
      }
      table={
        <DotTable
          items={items}
          format={cost.exact}
          caption="Effective cost per million output tokens"
          header="Model"
          valueHeader="Per 1M output"
        />
      }
    />
  );
}

export function WaterfallCard({
  data,
  dimension,
  onDimension,
}: {
  data: Cost;
  dimension: BreakdownDimension;
  onDimension: (value: BreakdownDimension) => void;
}) {
  const theme = useChartTheme();
  const waterfall = data.waterfalls.find((w) => w.dimension === dimension) ?? data.waterfalls[0];
  if (waterfall === undefined) return null;
  const bars = waterfallBars(waterfall, theme);
  return (
    <ChartCard
      title="Change from the previous period"
      definition={data.definitions.waterfall}
      accuracy="estimated"
      actions={
        <DimensionTabs
          value={waterfall.dimension}
          onChange={onDimension}
          only={data.waterfalls.map((w) => w.dimension)}
        />
      }
      chart={
        <WaterfallChart
          bars={bars}
          format={cost.format}
          signed={signedCost}
          label="Change in cost from the previous period"
        />
      }
      table={
        <WaterfallTable
          bars={bars}
          format={cost.exact}
          signed={signedCost}
          caption="Change in cost from the previous period"
        />
      }
    />
  );
}
