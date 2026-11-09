import { useMemo } from "react";

import { fmt } from "../format";
import { Chart, baseOption, legendStyle, tooltipStyle, type ChartOption } from "./chart";
import { DataTable } from "./table";
import { useChartTheme, type ChartTheme } from "./theme";
import { tooltipContent, type TooltipLine } from "./tooltip";

export interface TrendSeries {
  name: string;
  color: string;
  values: readonly number[];
}

interface AxisParam {
  axisValue?: string;
  color?: string;
  seriesName?: string;
  value?: number;
}

function tooltip(params: unknown, format: (v: number) => string): HTMLElement | string {
  const list = (Array.isArray(params) ? params : [params]) as AxisParam[];
  const first = list[0];
  if (first === undefined) return "";
  const total = list.reduce((sum, p) => sum + (p.value ?? 0), 0);
  const lines: TooltipLine[] = list
    .filter((p) => (p.value ?? 0) > 0)
    .map((p) => ({
      label: p.seriesName ?? "",
      value: format(p.value ?? 0),
      ...(p.color === undefined ? {} : { color: p.color }),
    }));
  if (list.length > 1) lines.push({ label: "Total", value: format(total) });
  return tooltipContent(fmt.day(first.axisValue ?? ""), lines);
}

function trendOption(
  theme: ChartTheme,
  dates: readonly string[],
  series: readonly TrendSeries[],
  format: (v: number) => string,
): ChartOption {
  const area = dates.length > 31;
  return {
    ...baseOption(theme),
    legend: series.length > 1 ? legendStyle(theme, "roundRect") : { show: false },
    grid: { left: 4, right: 8, top: series.length > 1 ? 32 : 12, bottom: 4, containLabel: true },
    xAxis: {
      type: "category",
      data: dates,
      axisLine: { lineStyle: { color: theme.border } },
      axisTick: { show: false },
      axisLabel: { color: theme.subtle, formatter: (v: string) => v.slice(5).replace("-", "/") },
    },
    yAxis: {
      type: "value",
      splitLine: { lineStyle: { color: theme.border } },
      axisLabel: { color: theme.subtle, formatter: (v: number) => format(v) },
    },
    tooltip: {
      ...tooltipStyle(theme),
      trigger: "axis",
      formatter: (p: unknown) => tooltip(p, format),
    },
    series: series.map((s) => ({
      name: s.name,
      type: area ? "line" : "bar",
      stack: "total",
      data: s.values,
      itemStyle: { color: s.color, borderRadius: area ? 0 : [2, 2, 0, 0] },
      ...(area
        ? {
            areaStyle: { opacity: 0.22 },
            symbol: "none",
            lineStyle: { width: 1.5, color: s.color },
          }
        : { barMaxWidth: 28 }),
      emphasis: { focus: "series" },
    })),
  };
}

/** Stacked bars (≤ 31 days) or stacked area (longer) over local days; click selects a day. */
export function StackedTrend({
  dates,
  series,
  format,
  label,
  onSelectDate,
}: {
  dates: readonly string[];
  series: readonly TrendSeries[];
  format: (v: number) => string;
  label: string;
  onSelectDate?: (date: string) => void;
}) {
  const theme = useChartTheme();
  const option = useMemo(
    () => trendOption(theme, dates, series, format),
    [theme, dates, series, format],
  );
  return (
    <Chart
      option={option}
      label={label}
      height={280}
      onClick={
        onSelectDate === undefined
          ? undefined
          : (p) => {
              if (typeof p.name === "string") onSelectDate(p.name);
            }
      }
    />
  );
}

/** The table twin of {@link StackedTrend}: one row per day, one column per series. */
export function TrendTable({
  dates,
  series,
  format,
  caption,
}: {
  dates: readonly string[];
  series: readonly TrendSeries[];
  format: (v: number) => string;
  caption: string;
}) {
  const rows = dates.map((date, i) => ({ date, values: series.map((s) => s.values[i] ?? 0) }));
  return (
    <DataTable
      caption={caption}
      rows={[...rows].reverse()}
      rowKey={(r) => r.date}
      columns={[
        { header: "Day", cell: (r) => fmt.day(r.date) },
        ...series.map((s, i) => ({
          header: s.name,
          numeric: true,
          cell: (r: (typeof rows)[number]) => format(r.values[i] ?? 0),
        })),
        ...(series.length > 1
          ? [
              {
                header: "Total",
                numeric: true,
                cell: (r: (typeof rows)[number]) => format(r.values.reduce((a, b) => a + b, 0)),
              },
            ]
          : []),
      ]}
    />
  );
}
