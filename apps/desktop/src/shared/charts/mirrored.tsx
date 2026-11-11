/**
 * Two opposing quantities over local days: one drawn above the axis, the other below
 * it, so their balance reads at a glance. Axis labels and tooltips show magnitudes.
 */
import { useMemo } from "react";

import { fmt } from "../format";
import { dateAxis, valueAxis } from "./axes";
import { Chart, baseOption, legendStyle, tooltipStyle, type ChartOption } from "./chart";
import { DataTable } from "./table";
import { useChartTheme, type ChartTheme } from "./theme";
import { tooltipContent } from "./tooltip";

export interface MirroredSeries {
  name: string;
  color: string;
  values: readonly number[];
}

interface AxisParam {
  dataIndex?: number;
}

function areaSeries(series: MirroredSeries, sign: 1 | -1) {
  return {
    name: series.name,
    type: "line",
    data: series.values.map((v) => sign * v),
    symbol: "none",
    itemStyle: { color: series.color },
    lineStyle: { width: 1.5, color: series.color },
    areaStyle: { color: series.color, opacity: 0.22 },
  };
}

function mirroredOption(
  theme: ChartTheme,
  dates: readonly string[],
  above: MirroredSeries,
  below: MirroredSeries,
  format: (v: number) => string,
): ChartOption {
  return {
    ...baseOption(theme),
    legend: legendStyle(theme, "roundRect"),
    grid: { left: 4, right: 8, top: 32, bottom: 4, containLabel: true },
    xAxis: dateAxis(theme, dates),
    yAxis: valueAxis(theme, (v) => format(Math.abs(v))),
    tooltip: {
      ...tooltipStyle(theme),
      trigger: "axis",
      formatter: (params: unknown) => {
        const first = (Array.isArray(params) ? params : [params])[0] as AxisParam | undefined;
        const i = first?.dataIndex ?? -1;
        const date = dates[i];
        if (date === undefined) return "";
        return tooltipContent(fmt.day(date), [
          { label: above.name, value: format(above.values[i] ?? 0), color: above.color },
          { label: below.name, value: format(below.values[i] ?? 0), color: below.color },
        ]);
      },
    },
    series: [areaSeries(above, 1), areaSeries(below, -1)],
  };
}

export function MirroredTrend({
  dates,
  above,
  below,
  format,
  label,
}: {
  dates: readonly string[];
  above: MirroredSeries;
  below: MirroredSeries;
  format: (v: number) => string;
  label: string;
}) {
  const theme = useChartTheme();
  const option = useMemo(
    () => mirroredOption(theme, dates, above, below, format),
    [theme, dates, above, below, format],
  );
  return <Chart option={option} label={label} height={260} />;
}

/** The table twin of {@link MirroredTrend}. */
export function MirroredTable({
  dates,
  above,
  below,
  format,
  caption,
}: {
  dates: readonly string[];
  above: MirroredSeries;
  below: MirroredSeries;
  format: (v: number) => string;
  caption: string;
}) {
  const rows = dates.map((date, i) => ({
    date,
    above: above.values[i] ?? 0,
    below: below.values[i] ?? 0,
  }));
  return (
    <DataTable
      caption={caption}
      rows={[...rows].reverse()}
      rowKey={(r) => r.date}
      columns={[
        { header: "Day", cell: (r) => fmt.day(r.date) },
        { header: above.name, numeric: true, cell: (r) => format(r.above) },
        { header: below.name, numeric: true, cell: (r) => format(r.below) },
      ]}
    />
  );
}
