import { useMemo } from "react";

import { Chart, baseOption, tooltipStyle, type ChartOption } from "./chart";
import type { RankItem } from "./ranking";
import { DataTable } from "./table";
import { useChartTheme, type ChartTheme } from "./theme";
import { tooltipContent } from "./tooltip";

type Format = (v: number) => string;

function dotOption(theme: ChartTheme, items: readonly RankItem[], format: Format): ChartOption {
  // Drawn bottom-up; reverse so the first item is on top.
  const ordered = [...items].reverse();
  return {
    ...baseOption(theme),
    grid: { left: 4, right: 24, top: 8, bottom: 4, containLabel: true },
    xAxis: {
      type: "value",
      splitLine: { lineStyle: { color: theme.border } },
      axisLabel: { color: theme.subtle, formatter: (v: number) => format(v) },
    },
    yAxis: {
      type: "category",
      data: ordered.map((i) => i.label),
      axisLine: { show: false },
      axisTick: { show: false },
      splitLine: { show: true, lineStyle: { color: theme.border, type: "dotted" } },
      axisLabel: { color: theme.text, width: 160, overflow: "truncate" },
    },
    tooltip: {
      ...tooltipStyle(theme),
      trigger: "item",
      formatter: (p: unknown) => {
        const item = ordered[(p as { dataIndex?: number }).dataIndex ?? -1];
        if (item === undefined) return "";
        return tooltipContent(item.label, [{ label: format(item.value), color: item.color }]);
      },
    },
    series: [
      {
        type: "scatter",
        symbolSize: 11,
        data: ordered.map((i) => ({ value: i.value, itemStyle: { color: i.color } })),
      },
    ],
  };
}

/** A sorted dot plot: one dot per item on a shared scale, the first item on top. */
export function DotPlot({
  items,
  format,
  label,
}: {
  items: readonly RankItem[];
  format: Format;
  label: string;
}) {
  const theme = useChartTheme();
  const option = useMemo(() => dotOption(theme, items, format), [theme, items, format]);
  return <Chart option={option} label={label} height={Math.max(120, items.length * 28 + 32)} />;
}

/** The table twin of {@link DotPlot}. */
export function DotTable({
  items,
  format,
  caption,
  header,
  valueHeader,
}: {
  items: readonly RankItem[];
  format: Format;
  caption: string;
  header: string;
  valueHeader: string;
}) {
  return (
    <DataTable
      caption={caption}
      rows={items}
      rowKey={(r, i) => `${r.key ?? "none"}-${String(i)}`}
      columns={[
        { header, cell: (r) => <span className="break-all">{r.label}</span> },
        { header: valueHeader, numeric: true, cell: (r) => format(r.value) },
      ]}
    />
  );
}
