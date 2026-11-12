import type { McpServerRow, ToolRow, Tools, ToolsKpis } from "@veyra/contracts";

import {
  DataTable,
  RankTable,
  Ranking,
  StackedTrend,
  TrendTable,
  useChartTheme,
} from "@/shared/charts";
import { fmt } from "@/shared/format";
import { Card, ChartCard, EmptyState, KpiGrid, KpiTile } from "@/shared/ui";

import { KIND_LABEL, failureRate, kindSeries, toolItems, toolLabel } from "./tools-model";

const count = (v: number | null) => fmt.compact(v);
const exactCount = (v: number | null) => fmt.integer(v);
const percent = (v: number | null) => fmt.percent(v);

const TILES: readonly {
  key: keyof ToolsKpis;
  label: string;
  format: (v: number | null) => string;
  exact?: (v: number | null) => string;
}[] = [
  { key: "calls", label: "Tool calls", format: count, exact: exactCount },
  { key: "callsPerRequest", label: "Calls per request", format: (v) => fmt.times(v) },
  { key: "failureRate", label: "Failure rate", format: percent },
  { key: "mcpShare", label: "MCP share", format: percent },
  { key: "subagentShare", label: "Made by subagents", format: percent },
];

export function ToolsKpiStrip({ tools, trend }: { tools: Tools; trend: readonly number[] }) {
  return (
    <KpiGrid>
      {TILES.map((tile, i) => (
        <KpiTile
          key={tile.key}
          label={tile.label}
          kpi={tools.kpis[tile.key]}
          format={tile.format}
          {...(tile.exact === undefined ? {} : { exact: tile.exact })}
          hero={i === 0}
          {...(i === 0 ? { trend } : {})}
        />
      ))}
    </KpiGrid>
  );
}

export function KindsCard({
  tools,
  onSelectDate,
}: {
  tools: Tools;
  onSelectDate: (date: string) => void;
}) {
  const theme = useChartTheme();
  const { dates, series } = kindSeries(tools, theme);
  return (
    <ChartCard
      title="Tool calls by kind"
      definition="Tool calls per local day, by what the tool does. Each application names its tools differently; they are grouped so they compare. Click a day to narrow the period to it."
      chart={
        <StackedTrend
          dates={dates}
          series={series}
          format={count}
          label="Daily tool calls by kind"
          onSelectDate={onSelectDate}
        />
      }
      table={
        <TrendTable
          dates={dates}
          series={series}
          format={exactCount}
          caption="Daily tool calls by kind"
        />
      }
    />
  );
}

export function TopToolsCard({ tools }: { tools: Tools }) {
  const theme = useChartTheme();
  const items = toolItems(tools, theme);
  return (
    <ChartCard
      title="Most used tools"
      definition="The tools called most in the period, coloured by kind. An MCP tool shows its server in brackets."
      chart={<Ranking items={items} format={count} label="Most used tools" />}
      table={
        <RankTable items={items} format={exactCount} caption="Most used tools" header="Tool" />
      }
    />
  );
}

export function ServersCard({ tools }: { tools: Tools }) {
  return (
    <Card
      title="MCP servers"
      definition="Each MCP server whose tools were called, with how many of its tools were used and how often a call failed."
    >
      {tools.servers.length === 0 ? (
        <EmptyState title="No MCP server was used in this period" />
      ) : (
        <DataTable<McpServerRow>
          caption="MCP servers"
          rows={tools.servers}
          rowKey={(r) => r.server}
          columns={[
            { header: "Server", cell: (r) => <span className="break-all">{r.server}</span> },
            { header: "Tools used", numeric: true, cell: (r) => fmt.integer(r.tools) },
            { header: "Calls", numeric: true, cell: (r) => fmt.integer(r.counts.calls) },
            {
              header: "Failure rate",
              numeric: true,
              cell: (r) => fmt.percent(failureRate(r.counts)),
            },
          ]}
        />
      )}
    </Card>
  );
}

export function FailingCard({ tools }: { tools: Tools }) {
  return (
    <Card title="Tools that fail most" definition={tools.failureDefinition}>
      {tools.failing.length === 0 ? (
        <EmptyState title="No tool call failed in this period" />
      ) : (
        <DataTable<ToolRow>
          caption="Tools that fail most"
          rows={tools.failing}
          rowKey={(r) => r.name}
          columns={[
            { header: "Tool", cell: (r) => <span className="break-all">{toolLabel(r)}</span> },
            { header: "Kind", cell: (r) => KIND_LABEL[r.kind] },
            { header: "Failed", numeric: true, cell: (r) => fmt.integer(r.counts.failed) },
            { header: "Calls", numeric: true, cell: (r) => fmt.integer(r.counts.calls) },
            {
              header: "Failure rate",
              numeric: true,
              cell: (r) => fmt.percent(failureRate(r.counts)),
            },
          ]}
        />
      )}
    </Card>
  );
}
