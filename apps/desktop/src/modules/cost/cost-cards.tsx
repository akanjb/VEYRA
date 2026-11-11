/** The Cost page's headline and trend cards. */
import type { Cost } from "@veyra/contracts";
import { Link } from "@tanstack/react-router";

import { LineTable, LineTrend, StackedTrend, TrendTable, useChartTheme } from "@/shared/charts";
import { MEASURE, RECORDED_MONEY, fmt } from "@/shared/format";
import { Card, ChartCard, KpiGrid, KpiTile, cn } from "@/shared/ui";

import {
  LEVEL_LABEL,
  TILES,
  budgetUse,
  cumulativeInput,
  dailyInput,
  outlookInput,
  valueFraming,
  type TileFormat,
} from "./cost-model";

const cost = MEASURE.cost;

const FORMATS: Readonly<
  Record<TileFormat, { format: (v: number | null) => string; exact?: (v: number | null) => string }>
> = {
  estimate: { format: cost.format, exact: cost.exact },
  recorded: RECORDED_MONEY,
  ratio: { format: (v) => fmt.percent(v) },
};

/** The three cost kinds as separate tiles, with the estimate's ratios. Never a total. */
export function CostKpiStrip({ data }: { data: Cost }) {
  const trend = data.daily.map((d) => d.apiEquivalentNanoUsd);
  return (
    <KpiGrid>
      {TILES.map((tile, i) => (
        <KpiTile
          key={tile.key}
          label={tile.label}
          kpi={data.kpis[tile.key]}
          {...FORMATS[tile.format]}
          hero={i === 0}
          {...(i === 0 ? { trend } : {})}
        />
      ))}
    </KpiGrid>
  );
}

/** "Subscription $X · usage would cost $Y at API rates": a comparison, never a sum. */
export function ValueFraming({ data }: { data: Cost }) {
  const framing = valueFraming(data);
  const notes = [
    data.otherCurrencyEntries > 0
      ? `${String(data.otherCurrencyEntries)} billing ${data.otherCurrencyEntries === 1 ? "entry is" : "entries are"} in another currency and not included.`
      : null,
  ].filter((n) => n !== null);
  if (framing === null && notes.length === 0) return null;
  return (
    <p className="text-sm text-muted" data-numeric>
      {framing === null ? null : (
        <>
          Subscriptions {RECORDED_MONEY.exact(framing.subscription)} · the same usage would cost{" "}
          {cost.exact(framing.apiEquivalent)} at API rates. These are compared, never added.{" "}
        </>
      )}
      {notes.join(" ")}
    </p>
  );
}

export function DailyCostCard({
  data,
  onSelectDate,
}: {
  data: Cost;
  onSelectDate: (date: string) => void;
}) {
  const theme = useChartTheme();
  const { dates, series, overlay } = dailyInput(data, theme);
  return (
    <ChartCard
      title="Daily cost"
      definition="API-equivalent cost per local day (bars) and its running total over the period (line, right axis). Click a day to narrow the period to it."
      accuracy="estimated"
      chart={
        <StackedTrend
          dates={dates}
          series={series}
          overlay={overlay}
          overlayAxis="own"
          format={cost.format}
          label="Daily API-equivalent cost and running total"
          onSelectDate={onSelectDate}
        />
      }
      table={
        <TrendTable
          dates={dates}
          series={series}
          overlay={overlay}
          format={cost.exact}
          caption="Daily API-equivalent cost and running total"
        />
      }
    />
  );
}

export function CumulativeCard({ data }: { data: Cost }) {
  const theme = useChartTheme();
  const { dates, series } = cumulativeInput(data, theme);
  return (
    <ChartCard
      title="This period against the previous one"
      definition="Running API-equivalent cost of this period and of the previous period of the same length, aligned by day."
      accuracy="estimated"
      chart={
        <LineTrend
          dates={dates}
          series={series}
          format={cost.format}
          label="Running cost of this period and the previous one"
        />
      }
      table={
        <LineTable
          dates={dates}
          series={series}
          format={cost.exact}
          caption="Running cost of this period and the previous one"
        />
      }
    />
  );
}

export function MonthOutlookCard({ data }: { data: Cost }) {
  const theme = useChartTheme();
  const { dates, series, thresholds } = outlookInput(data, theme);
  const { month, definitions } = data;
  return (
    <ChartCard
      title={`Month outlook · ${fmt.month(month.month)}`}
      definition={`${definitions.forecast} ${definitions.runRate}`}
      accuracy="estimated"
      actions={
        <span className="text-xs text-muted" data-numeric>
          Run rate {cost.format(month.runRateNanoUsd)}/day · forecast{" "}
          {cost.format(month.forecastNanoUsd)}
        </span>
      }
      chart={
        <LineTrend
          dates={dates}
          series={series}
          thresholds={thresholds}
          format={cost.format}
          label="Month-to-date cost, projection and budget limits"
        />
      }
      table={
        <LineTable
          dates={dates}
          series={series}
          format={cost.exact}
          caption="Month-to-date cost and projection"
        />
      }
    />
  );
}

const LEVEL_STYLE = {
  within: "bg-success",
  warning: "bg-warning",
  over: "bg-danger",
} as const;

export function BudgetsCard({ data }: { data: Cost }) {
  return (
    <Card
      title="Budgets"
      definition={data.definitions.budget}
      accuracy="estimated"
      actions={
        <Link to="/settings" className="text-xs text-muted hover:text-text">
          Manage
        </Link>
      }
    >
      {data.budgets.length === 0 ? (
        <p className="text-sm text-muted">
          No budgets yet. Add one in{" "}
          <Link to="/settings" className="underline">
            Settings
          </Link>{" "}
          to track this month against a limit.
        </p>
      ) : (
        <ul className="grid gap-3">
          {data.budgets.map((b) => (
            <li key={b.id} className="grid gap-1 text-sm">
              <div className="flex flex-wrap items-baseline gap-2">
                <span className="font-medium">{b.label}</span>
                <span className="text-xs text-muted">{b.project?.label ?? "All usage"}</span>
                <span className="ml-auto text-xs" data-numeric>
                  {cost.format(b.monthToDateNanoUsd)} of {RECORDED_MONEY.format(b.limitNanoUsd)}
                </span>
              </div>
              <div
                role="meter"
                aria-label={`${b.label}: ${LEVEL_LABEL[b.level]}`}
                aria-valuemin={0}
                aria-valuemax={b.limitNanoUsd}
                aria-valuenow={Math.min(b.monthToDateNanoUsd, b.limitNanoUsd)}
                className="h-2 overflow-hidden rounded-full bg-gray-3"
              >
                <div
                  className={cn("h-full", LEVEL_STYLE[b.level])}
                  style={{ width: `${String(budgetUse(b) * 100)}%` }}
                />
              </div>
              <p className="text-xs text-muted">
                {LEVEL_LABEL[b.level]} · forecast {cost.format(b.forecastNanoUsd)} (
                {LEVEL_LABEL[b.forecastLevel].toLowerCase()})
              </p>
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}
