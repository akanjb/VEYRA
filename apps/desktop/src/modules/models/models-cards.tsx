import type { ModelRow, Models } from "@veyra/contracts";

import {
  DataTable,
  HeatTable,
  Lanes,
  LanesTable,
  Ranking,
  StackedTrend,
  TrendTable,
  useChartTheme,
} from "@/shared/charts";
import { MEASURE, fmt } from "@/shared/format";
import { Card, CardGrid, ChartCard } from "@/shared/ui";

import {
  COMPARED,
  COMPARE_MAX,
  COMPARE_MIN,
  comparedItems,
  heatMatrix,
  lanes,
  rowId,
  shareInput,
} from "./models-model";

const tokens = MEASURE.tokens;
const money = MEASURE.cost;
const percent = (v: number) => fmt.percent(v);

export function ShareCard({ models }: { models: Models }) {
  const theme = useChartTheme();
  const { dates, series } = shareInput(models, theme);
  return (
    <ChartCard
      title="Provider share over time"
      definition="Each provider's share of the tokens of each local day."
      accuracy={models.accuracy}
      chart={
        <StackedTrend
          dates={dates}
          series={series}
          format={percent}
          label="Provider share per day"
        />
      }
      table={
        <TrendTable
          dates={dates}
          series={series}
          format={percent}
          caption="Provider share per day"
        />
      }
    />
  );
}

export function LeaderboardCard({
  models,
  compared,
  onToggle,
}: {
  models: Models;
  compared: readonly string[];
  onToggle: (id: string) => void;
}) {
  return (
    <Card
      title="Model leaderboard"
      definition={`Every model used in the period, largest first. Tick ${String(COMPARE_MIN)} to ${String(COMPARE_MAX)} models to compare them. ${models.rateDefinition}`}
      accuracy={models.accuracy}
    >
      <div className="max-h-[28rem] overflow-auto">
        <DataTable<ModelRow>
          caption="Model leaderboard"
          rows={models.leaderboard}
          rowKey={rowId}
          columns={[
            {
              header: "Compare",
              cell: (r) => {
                const id = rowId(r);
                const checked = compared.includes(id);
                return (
                  <input
                    type="checkbox"
                    aria-label={`Compare ${r.label}`}
                    checked={checked}
                    disabled={!checked && compared.length >= COMPARE_MAX}
                    onChange={() => {
                      onToggle(id);
                    }}
                  />
                );
              },
            },
            { header: "Model", cell: (r) => <span className="break-all">{r.label}</span> },
            { header: "Provider", cell: (r) => r.providerLabel },
            { header: "Tokens", numeric: true, cell: (r) => tokens.format(r.measures.tokens) },
            { header: "Requests", numeric: true, cell: (r) => fmt.integer(r.measures.requests) },
            { header: "Cost", numeric: true, cell: (r) => money.format(r.measures.costNanoUsd) },
            {
              header: "Per 1M output",
              numeric: true,
              cell: (r) => money.exact(r.costPerMillionOutputNanoUsd),
            },
            { header: "Cache hits", numeric: true, cell: (r) => fmt.percent(r.cacheHitRatio) },
            { header: "Priced", numeric: true, cell: (r) => fmt.percent(r.pricingCoverage) },
            { header: "First used", cell: (r) => fmt.day(r.firstDay) },
            { header: "Last used", cell: (r) => fmt.day(r.lastDay) },
          ]}
        />
      </div>
    </Card>
  );
}

export function CompareCard({ rows }: { rows: readonly ModelRow[] }) {
  const theme = useChartTheme();
  const format = { tokens: tokens.format, money: money.exact, ratio: percent } as const;
  return (
    <Card
      title="Model comparison"
      definition="The ticked models side by side, one measure per chart. Bars are used rather than a radar chart, which distorts comparisons."
      accuracy={null}
    >
      <CardGrid>
        {COMPARED.map((measure) => (
          <div key={measure.title} className="grid gap-1">
            <h3 className="text-xs font-medium text-muted">{measure.title}</h3>
            <Ranking
              items={comparedItems(rows, measure.pick, theme)}
              format={(v) => format[measure.kind](v)}
              label={`${measure.title} of the compared models`}
              barLabel="value"
            />
          </div>
        ))}
      </CardGrid>
    </Card>
  );
}

export function MatrixCard({
  title,
  definition,
  corner,
  matrix,
  accuracy,
}: {
  title: string;
  definition: string;
  corner: string;
  matrix: Models["providerByApp"];
  accuracy: Models["accuracy"];
}) {
  return (
    <Card title={title} definition={definition} accuracy={accuracy}>
      <HeatTable
        matrix={heatMatrix(matrix)}
        format={tokens.format}
        caption={title}
        corner={corner}
      />
    </Card>
  );
}

export function TimelineCard({ models }: { models: Models }) {
  const modelLanes = lanes(models);
  return (
    <ChartCard
      title="When each model was used"
      definition="Tokens per local day for the most used models; darker cells mean more tokens."
      accuracy={models.accuracy}
      chart={
        <Lanes
          dates={models.dates}
          lanes={modelLanes}
          format={tokens.format}
          label="Daily tokens per model"
        />
      }
      table={
        <LanesTable
          dates={models.dates}
          lanes={modelLanes}
          format={tokens.exact}
          caption="Daily tokens per model"
          header="Model"
        />
      }
    />
  );
}
