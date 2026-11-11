import { useMemo } from "react";

import { MISSING, fmt } from "../format";
import { dateAxis, valueAxis } from "./axes";
import { Chart, baseOption, legendStyle, tooltipStyle, type ChartOption } from "./chart";
import { DataTable } from "./table";
import { useChartTheme, type ChartTheme } from "./theme";
import { tooltipContent, type TooltipLine } from "./tooltip";

export interface LineSeries {
  name: string;
  color: string;
  /** `null` where the series has no value (drawn as a gap). */
  values: readonly (number | null)[];
  /** Drawn dashed, e.g. a projection. */
  dashed?: boolean;
}

/** A horizontal reference line, e.g. a budget limit. */
export interface Threshold {
  label: string;
  value: number;
  color: string;
}

type Format = (v: number) => string;

interface AxisParam {
  axisValue?: string;
  color?: string;
  seriesName?: string;
  value?: number | null;
}

function tooltip(params: unknown, format: Format): HTMLElement | string {
  const list = (Array.isArray(params) ? params : [params]) as AxisParam[];
  const first = list[0];
  if (first === undefined) return "";
  const lines: TooltipLine[] = list
    .filter((p) => p.value !== null && p.value !== undefined)
    .map((p) => ({
      label: p.seriesName ?? "",
      value: format(p.value ?? 0),
      ...(p.color === undefined ? {} : { color: p.color }),
    }));
  return tooltipContent(fmt.day(first.axisValue ?? ""), lines);
}

function thresholdLines(thresholds: readonly Threshold[], format: Format) {
  return {
    silent: true,
    symbol: "none",
    data: thresholds.map((t) => ({
      yAxis: t.value,
      lineStyle: { color: t.color, type: "dotted", width: 1.5 },
      label: {
        formatter: `${t.label} ${format(t.value)}`,
        color: t.color,
        position: "insideEndTop",
      },
    })),
  };
}

function lineOption(
  theme: ChartTheme,
  dates: readonly string[],
  series: readonly LineSeries[],
  format: Format,
  thresholds: readonly Threshold[],
): ChartOption {
  const highest = Math.max(0, ...thresholds.map((t) => t.value));
  return {
    ...baseOption(theme),
    legend: series.length > 1 ? legendStyle(theme, "roundRect") : { show: false },
    grid: { left: 4, right: 8, top: series.length > 1 ? 32 : 12, bottom: 4, containLabel: true },
    xAxis: dateAxis(theme, dates),
    // Thresholds above the data stay visible.
    yAxis: { ...valueAxis(theme, format), max: (v: { max: number }) => Math.max(v.max, highest) },
    tooltip: {
      ...tooltipStyle(theme),
      trigger: "axis",
      formatter: (p: unknown) => tooltip(p, format),
    },
    series: series.map((s, i) => ({
      name: s.name,
      type: "line",
      data: s.values,
      symbol: "none",
      connectNulls: false,
      itemStyle: { color: s.color },
      lineStyle: { width: 2, color: s.color, type: s.dashed === true ? "dashed" : "solid" },
      ...(i === 0 && thresholds.length > 0 ? { markLine: thresholdLines(thresholds, format) } : {}),
    })),
  };
}

/** Lines over local days, with optional dashed series and threshold lines. */
export function LineTrend({
  dates,
  series,
  format,
  label,
  thresholds = [],
}: {
  dates: readonly string[];
  series: readonly LineSeries[];
  format: Format;
  label: string;
  thresholds?: readonly Threshold[];
}) {
  const theme = useChartTheme();
  const option = useMemo(
    () => lineOption(theme, dates, series, format, thresholds),
    [theme, dates, series, format, thresholds],
  );
  return <Chart option={option} label={label} height={260} />;
}

/** The table twin of {@link LineTrend}: one row per day, one column per series. */
export function LineTable({
  dates,
  series,
  format,
  caption,
}: {
  dates: readonly string[];
  series: readonly LineSeries[];
  format: Format;
  caption: string;
}) {
  const rows = dates.map((date, i) => ({ date, values: series.map((s) => s.values[i] ?? null) }));
  type Row = (typeof rows)[number];
  return (
    <DataTable
      caption={caption}
      rows={[...rows].reverse()}
      rowKey={(r) => r.date}
      columns={[
        { header: "Day", cell: (r: Row) => fmt.day(r.date) },
        ...series.map((s, i) => ({
          header: s.name,
          numeric: true,
          cell: (r: Row) => {
            const value = r.values[i];
            return value === null || value === undefined ? MISSING : format(value);
          },
        })),
      ]}
    />
  );
}
