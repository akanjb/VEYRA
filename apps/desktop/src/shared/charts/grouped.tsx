/** Several quantities side by side for each category, such as writes per TTL per model. */
import { useMemo } from "react";

import { categoryAxis, valueAxis } from "./axes";
import { Chart, baseOption, legendStyle, tooltipStyle, type ChartOption } from "./chart";
import { DataTable } from "./table";
import { useChartTheme, type ChartTheme } from "./theme";
import { tooltipContent } from "./tooltip";

export interface GroupSeries {
  name: string;
  color: string;
  /** One value per category. */
  values: readonly number[];
}

interface AxisParam {
  dataIndex?: number;
}

function groupedOption(
  theme: ChartTheme,
  categories: readonly string[],
  series: readonly GroupSeries[],
  format: (v: number) => string,
): ChartOption {
  return {
    ...baseOption(theme),
    legend: legendStyle(theme, "roundRect"),
    grid: { left: 4, right: 8, top: 32, bottom: 4, containLabel: true },
    xAxis: categoryAxis(theme, categories),
    yAxis: valueAxis(theme, format),
    tooltip: {
      ...tooltipStyle(theme),
      trigger: "axis",
      formatter: (params: unknown) => {
        const first = (Array.isArray(params) ? params : [params])[0] as AxisParam | undefined;
        const i = first?.dataIndex ?? -1;
        const category = categories[i];
        if (category === undefined) return "";
        return tooltipContent(
          category,
          series.map((s) => ({ label: s.name, value: format(s.values[i] ?? 0), color: s.color })),
        );
      },
    },
    series: series.map((s) => ({
      name: s.name,
      type: "bar",
      data: s.values,
      barMaxWidth: 18,
      itemStyle: { color: s.color, borderRadius: [2, 2, 0, 0] },
    })),
  };
}

export function GroupedBars({
  categories,
  series,
  format,
  label,
}: {
  categories: readonly string[];
  series: readonly GroupSeries[];
  format: (v: number) => string;
  label: string;
}) {
  const theme = useChartTheme();
  const option = useMemo(
    () => groupedOption(theme, categories, series, format),
    [theme, categories, series, format],
  );
  return <Chart option={option} label={label} height={260} />;
}

/** The table twin of {@link GroupedBars}: one row per category, one column per series. */
export function GroupedTable({
  categories,
  series,
  format,
  caption,
  header,
}: {
  categories: readonly string[];
  series: readonly GroupSeries[];
  format: (v: number) => string;
  caption: string;
  header: string;
}) {
  const rows = categories.map((category, i) => ({ category, i }));
  return (
    <DataTable
      caption={caption}
      rows={rows}
      rowKey={(r) => `${r.category}-${String(r.i)}`}
      columns={[
        { header, cell: (r) => <span className="break-all">{r.category}</span> },
        ...series.map((s) => ({
          header: s.name,
          numeric: true,
          cell: (r: (typeof rows)[number]) => format(s.values[r.i] ?? 0),
        })),
      ]}
    />
  );
}
