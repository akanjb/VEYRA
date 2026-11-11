/**
 * A daily statistic with its spread: the median as a line, the band from the median to
 * the 90th percentile, and the mean as a dashed line.
 */
import { useMemo } from "react";

import { MISSING, fmt } from "../format";
import { dateAxis, valueAxis } from "./axes";
import { Chart, baseOption, legendStyle, tooltipStyle, type ChartOption } from "./chart";
import { DataTable } from "./table";
import { useChartTheme, type ChartTheme } from "./theme";
import { tooltipContent } from "./tooltip";

/** One day's statistics; `null` where the day has no data. */
export interface BandPoint {
  date: string;
  count: number;
  mean: number | null;
  p50: number | null;
  p90: number | null;
}

const BASE = "band-base";
const SPREAD = "band-spread";

interface AxisParam {
  dataIndex?: number;
}

interface BandColors {
  median: string;
  mean: string;
}

function bandTooltip(
  points: readonly BandPoint[],
  format: (v: number) => string,
  counted: string,
  colors: BandColors,
) {
  return (params: unknown) => {
    const first = (Array.isArray(params) ? params : [params])[0] as AxisParam | undefined;
    const point = points[first?.dataIndex ?? -1];
    if (point === undefined) return "";
    const value = (v: number | null) => (v === null ? MISSING : format(v));
    return tooltipContent(fmt.day(point.date), [
      { label: "P90", value: value(point.p90) },
      { label: "Median (P50)", value: value(point.p50), color: colors.median },
      { label: "Mean", value: value(point.mean), color: colors.mean },
      { label: counted, value: fmt.integer(point.count) },
    ]);
  };
}

/** An invisible base at the median, the stacked spread to P90, then the two lines. */
function bandSeries(points: readonly BandPoint[], { median, mean }: BandColors) {
  const line = { type: "line", symbol: "none" };
  return [
    {
      ...line,
      id: BASE,
      stack: "band",
      data: points.map((p) => p.p50),
      lineStyle: { opacity: 0 },
      silent: true,
    },
    {
      ...line,
      id: SPREAD,
      name: "P50–P90",
      stack: "band",
      data: points.map((p) => (p.p50 === null || p.p90 === null ? null : p.p90 - p.p50)),
      lineStyle: { opacity: 0 },
      itemStyle: { color: median },
      areaStyle: { color: median, opacity: 0.18 },
      silent: true,
    },
    {
      ...line,
      name: "Median (P50)",
      data: points.map((p) => p.p50),
      itemStyle: { color: median },
      lineStyle: { width: 2, color: median },
    },
    {
      ...line,
      name: "Mean",
      data: points.map((p) => p.mean),
      itemStyle: { color: mean },
      lineStyle: { width: 1.5, color: mean, type: "dashed" },
    },
  ];
}

function bandOption(
  theme: ChartTheme,
  points: readonly BandPoint[],
  format: (v: number) => string,
  counted: string,
): ChartOption {
  const colors = { median: theme.data[0] ?? theme.accent, mean: theme.data[4] ?? theme.text };
  return {
    ...baseOption(theme),
    legend: { ...legendStyle(theme, "roundRect"), data: ["Median (P50)", "P50–P90", "Mean"] },
    grid: { left: 4, right: 8, top: 32, bottom: 4, containLabel: true },
    xAxis: dateAxis(
      theme,
      points.map((p) => p.date),
    ),
    yAxis: valueAxis(theme, format),
    tooltip: {
      ...tooltipStyle(theme),
      trigger: "axis",
      formatter: bandTooltip(points, format, counted, colors),
    },
    series: bandSeries(points, colors),
  };
}

/** Median, P50–P90 band and mean per day. `counted` names what `count` counts. */
export function BandTrend({
  points,
  format,
  label,
  counted,
}: {
  points: readonly BandPoint[];
  format: (v: number) => string;
  label: string;
  counted: string;
}) {
  const theme = useChartTheme();
  const option = useMemo(
    () => bandOption(theme, points, format, counted),
    [theme, points, format, counted],
  );
  return <Chart option={option} label={label} height={240} />;
}

/** The table twin of {@link BandTrend}. */
export function BandTable({
  points,
  format,
  caption,
  counted,
}: {
  points: readonly BandPoint[];
  format: (v: number) => string;
  caption: string;
  counted: string;
}) {
  const value = (v: number | null) => (v === null ? MISSING : format(v));
  return (
    <DataTable
      caption={caption}
      rows={[...points].reverse()}
      rowKey={(p) => p.date}
      columns={[
        { header: "Day", cell: (p) => fmt.day(p.date) },
        { header: counted, numeric: true, cell: (p) => fmt.integer(p.count) },
        { header: "Mean", numeric: true, cell: (p) => value(p.mean) },
        { header: "Median (P50)", numeric: true, cell: (p) => value(p.p50) },
        { header: "P90", numeric: true, cell: (p) => value(p.p90) },
      ]}
    />
  );
}
