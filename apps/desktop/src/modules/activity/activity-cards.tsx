import type { Activity, ActivityKpis, LongSession } from "@veyra/contracts";
import { useState } from "react";

import {
  DataTable,
  GroupedBars,
  GroupedTable,
  Histogram,
  HistogramTable,
  Ranking,
  useChartTheme,
} from "@/shared/charts";
import { MISSING, fmt } from "@/shared/format";
import { useOpenSession } from "@/shared/links";
import { Card, ChartCard, EmptyState, KpiGrid, KpiTile, Segmented } from "@/shared/ui";

import {
  busiestWeekdays,
  dailyInput,
  lengthBins,
  peakHours,
  type LengthMeasure,
} from "./activity-model";

const duration = (v: number | null) => fmt.duration(v);
const durationBound = (v: number) => fmt.duration(v);

const TILES: readonly {
  key: keyof ActivityKpis;
  label: string;
  format: (v: number | null) => string;
  derived: boolean;
}[] = [
  { key: "activeMs", label: "Active time", format: duration, derived: true },
  { key: "wallMs", label: "Wall time", format: duration, derived: true },
  { key: "sessions", label: "Sessions", format: (v) => fmt.integer(v), derived: false },
  { key: "activeDays", label: "Active days", format: (v) => fmt.integer(v), derived: false },
];

export function ActivityKpiStrip({ activity }: { activity: Activity }) {
  return (
    <KpiGrid>
      {TILES.map((tile, i) => (
        <KpiTile
          key={tile.key}
          label={tile.label}
          kpi={activity.kpis[tile.key]}
          format={tile.format}
          {...(tile.derived ? { accuracy: "derived" as const } : {})}
          hero={i === 0}
          {...(i === 0 ? { trend: activity.daily.map((d) => d.activeMs) } : {})}
        />
      ))}
    </KpiGrid>
  );
}

export function DailyTimeCard({ activity }: { activity: Activity }) {
  const theme = useChartTheme();
  const { categories, series } = dailyInput(activity, theme);
  return (
    <ChartCard
      title="Active and wall time per day"
      definition={`${activity.activeDefinition} ${activity.wallDefinition}`}
      accuracy="derived"
      chart={
        <GroupedBars
          categories={categories}
          series={series}
          format={durationBound}
          label="Active and wall time per day"
        />
      }
      table={
        <GroupedTable
          categories={categories}
          series={series}
          format={durationBound}
          caption="Active and wall time per day"
          header="Day"
        />
      }
    />
  );
}

export function PeaksCard({ activity }: { activity: Activity }) {
  const theme = useChartTheme();
  const hours = peakHours(activity, theme);
  const days = busiestWeekdays(activity, theme);
  return (
    <Card
      title="Peak hours and busiest days"
      definition="The local hours and weekdays with the most tokens over the period, with their share."
    >
      <div className="grid gap-4 md:grid-cols-2">
        <div className="grid gap-1">
          <h3 className="text-xs font-medium text-muted">Peak hours</h3>
          <Ranking
            items={hours}
            format={fmt.compact}
            label="Hours of the day with the most tokens"
          />
        </div>
        <div className="grid gap-1">
          <h3 className="text-xs font-medium text-muted">Busiest weekdays</h3>
          <Ranking items={days} format={fmt.compact} label="Weekdays with the most tokens" />
        </div>
      </div>
    </Card>
  );
}

const LENGTH_OPTIONS = [
  { value: "active", label: "Active time" },
  { value: "wall", label: "Wall time" },
] as const;

export function LengthsCard({ activity }: { activity: Activity }) {
  const [measure, setMeasure] = useState<LengthMeasure>("active");
  const bins = lengthBins(activity, measure);
  const title = "Session lengths";
  return (
    <ChartCard
      title={title}
      definition={`How many sessions fall in each length range, by ${measure} time. ${measure === "active" ? activity.activeDefinition : activity.wallDefinition}`}
      accuracy="derived"
      actions={
        <Segmented
          value={measure}
          options={LENGTH_OPTIONS}
          onChange={setMeasure}
          label="Session length by"
        />
      }
      chart={
        <Histogram
          bins={bins}
          label={`Sessions by ${measure} time`}
          counted="Sessions"
          bound={durationBound}
          unit=""
        />
      }
      table={
        <HistogramTable
          bins={bins}
          caption={`Sessions by ${measure} time`}
          counted="Sessions"
          bound={durationBound}
          header="Length"
        />
      }
    />
  );
}

export function LongestCard({ activity }: { activity: Activity }) {
  const open = useOpenSession();
  const rows = activity.longest;
  return (
    <Card title="Longest sessions" definition={activity.wallDefinition} accuracy="derived">
      {rows.length === 0 ? (
        <EmptyState title="No sessions in this period" />
      ) : (
        <DataTable<LongSession>
          caption="Longest sessions"
          rows={rows}
          rowKey={(r) => String(r.id)}
          onRowClick={(r) => {
            open(r.id);
          }}
          columns={[
            { header: "Started", cell: (r) => fmt.dateTime(r.startedAt) },
            {
              header: "Project",
              cell: (r) => (r.project === null ? MISSING : fmt.basename(r.project)),
            },
            { header: "Application", cell: (r) => r.app },
            { header: "Models", cell: (r) => r.models.map((m) => m.label).join(", ") },
            { header: "Wall time", numeric: true, cell: (r) => fmt.duration(r.wallMs) },
            { header: "Active time", numeric: true, cell: (r) => fmt.duration(r.activeMs) },
          ]}
        />
      )}
    </Card>
  );
}
