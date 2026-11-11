/**
 * A Pareto chart: items as bars in descending order, with the running share of the
 * total as a line on a 0–100% axis.
 */
import { useMemo } from "react";

import { fmt } from "../format";
import { categoryAxis, shareAxis, valueAxis } from "./axes";
import { Chart, baseOption, tooltipStyle, type ChartOption } from "./chart";
import type { RankItem } from "./ranking";
import { DataTable } from "./table";
import { useChartTheme, type ChartTheme } from "./theme";
import { tooltipContent } from "./tooltip";

type Format = (v: number) => string;

interface AxisParam {
  dataIndex?: number;
}

function paretoOption(
  theme: ChartTheme,
  items: readonly RankItem[],
  cumulative: readonly number[],
  format: Format,
): ChartOption {
  return {
    ...baseOption(theme),
    grid: { left: 4, right: 4, top: 12, bottom: 4, containLabel: true },
    xAxis: {
      ...categoryAxis(
        theme,
        items.map((i) => i.label),
      ),
      axisLabel: { show: false },
    },
    yAxis: [valueAxis(theme, format), shareAxis(theme, (v) => fmt.percent(v))],
    tooltip: {
      ...tooltipStyle(theme),
      trigger: "axis",
      formatter: (params: unknown) => {
        const first = (Array.isArray(params) ? params : [params])[0] as AxisParam | undefined;
        const index = first?.dataIndex ?? -1;
        const item = items[index];
        if (item === undefined) return "";
        return tooltipContent(item.label, [
          { label: format(item.value), value: fmt.percent(item.share), color: item.color },
          { label: "Running share", value: fmt.percent(cumulative[index] ?? 0) },
        ]);
      },
    },
    series: [
      {
        type: "bar",
        data: items.map((i) => ({ value: i.value, itemStyle: { color: i.color } })),
        barCategoryGap: "12%",
        itemStyle: { borderRadius: [2, 2, 0, 0] },
      },
      {
        type: "line",
        yAxisIndex: 1,
        data: cumulative,
        symbol: "circle",
        symbolSize: 5,
        itemStyle: { color: theme.text },
        lineStyle: { width: 1.5, color: theme.text },
      },
    ],
  };
}

export function Pareto({
  items,
  cumulative,
  format,
  label,
}: {
  items: readonly RankItem[];
  /** The running share after each item, aligned with `items`. */
  cumulative: readonly number[];
  format: Format;
  label: string;
}) {
  const theme = useChartTheme();
  const option = useMemo(
    () => paretoOption(theme, items, cumulative, format),
    [theme, items, cumulative, format],
  );
  return <Chart option={option} label={label} height={240} />;
}

/** The table twin of {@link Pareto}. */
export function ParetoTable({
  items,
  cumulative,
  format,
  caption,
  header,
}: {
  items: readonly RankItem[];
  cumulative: readonly number[];
  format: Format;
  caption: string;
  header: string;
}) {
  const rows = items.map((item, i) => ({ item, running: cumulative[i] ?? 0 }));
  return (
    <DataTable
      caption={caption}
      rows={rows}
      rowKey={(r, i) => `${r.item.key ?? "none"}-${String(i)}`}
      columns={[
        { header, cell: (r) => <span className="break-all">{r.item.label}</span> },
        { header: "Value", numeric: true, cell: (r) => format(r.item.value) },
        { header: "Share", numeric: true, cell: (r) => fmt.percent(r.item.share) },
        { header: "Running share", numeric: true, cell: (r) => fmt.percent(r.running) },
      ]}
    />
  );
}
