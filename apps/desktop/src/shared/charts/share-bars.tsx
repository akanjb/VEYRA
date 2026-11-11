import { useMemo } from "react";

import { fmt } from "../format";
import { Chart, baseOption, legendStyle, tooltipStyle, type ChartOption } from "./chart";
import { DataTable } from "./table";
import { useChartTheme, type ChartTheme } from "./theme";
import { tooltipContent } from "./tooltip";

/** One part of every row, e.g. a token kind. */
export interface SharePart {
  name: string;
  color: string;
}

/** One bar: its amount of each part, aligned with the parts. */
export interface ShareRow {
  label: string;
  amounts: readonly number[];
}

type Format = (v: number) => string;

/** Each amount as a share (0–1) of its row's total; all zero for an empty row. */
export function rowShares(row: ShareRow): number[] {
  const total = row.amounts.reduce((sum, a) => sum + a, 0);
  return row.amounts.map((a) => (total === 0 ? 0 : a / total));
}

function shareOption(
  theme: ChartTheme,
  parts: readonly SharePart[],
  rows: readonly ShareRow[],
  format: Format,
): ChartOption {
  // Bars are drawn bottom-up; reverse so the first row is on top.
  const ordered = [...rows].reverse();
  const shares = ordered.map(rowShares);
  return {
    ...baseOption(theme),
    legend: legendStyle(theme, "roundRect"),
    grid: { left: 4, right: 12, top: 32, bottom: 4, containLabel: true },
    xAxis: {
      type: "value",
      max: 1,
      splitLine: { lineStyle: { color: theme.border } },
      axisLabel: { color: theme.subtle, formatter: (v: number) => fmt.percent(v) },
    },
    yAxis: {
      type: "category",
      data: ordered.map((r) => r.label),
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: { color: theme.text, width: 160, overflow: "truncate" },
    },
    tooltip: {
      ...tooltipStyle(theme),
      trigger: "item",
      formatter: (p: unknown) => {
        const { dataIndex = -1, seriesIndex = -1 } = p as {
          dataIndex?: number;
          seriesIndex?: number;
        };
        const row = ordered[dataIndex];
        const part = parts[seriesIndex];
        if (row === undefined || part === undefined) return "";
        return tooltipContent(row.label, [
          {
            label: part.name,
            value: `${format(row.amounts[seriesIndex] ?? 0)} · ${fmt.percent(shares[dataIndex]?.[seriesIndex] ?? 0)}`,
            color: part.color,
          },
        ]);
      },
    },
    series: parts.map((part, i) => ({
      name: part.name,
      type: "bar",
      stack: "share",
      barMaxWidth: 22,
      itemStyle: { color: part.color },
      data: shares.map((s) => s[i] ?? 0),
    })),
  };
}

/** 100% stacked horizontal bars: how each row's total divides into parts. */
export function ShareBars({
  parts,
  rows,
  format,
  label,
}: {
  parts: readonly SharePart[];
  rows: readonly ShareRow[];
  /** Formats an amount, shown in tooltips next to its share. */
  format: Format;
  label: string;
}) {
  const theme = useChartTheme();
  const option = useMemo(
    () => shareOption(theme, parts, rows, format),
    [theme, parts, rows, format],
  );
  return <Chart option={option} label={label} height={Math.max(140, rows.length * 32 + 48)} />;
}

/** The table twin of {@link ShareBars}: each part's amount and share per row. */
export function ShareTable({
  parts,
  rows,
  format,
  caption,
  header,
}: {
  parts: readonly SharePart[];
  rows: readonly ShareRow[];
  format: Format;
  caption: string;
  header: string;
}) {
  return (
    <DataTable
      caption={caption}
      rows={rows}
      rowKey={(r, i) => `${r.label}-${String(i)}`}
      columns={[
        { header, cell: (r) => <span className="break-all">{r.label}</span> },
        ...parts.map((part, i) => ({
          header: part.name,
          numeric: true,
          cell: (r: ShareRow) =>
            `${format(r.amounts[i] ?? 0)} (${fmt.percent(rowShares(r)[i] ?? 0)})`,
        })),
      ]}
    />
  );
}
