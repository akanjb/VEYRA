/**
 * A distribution over size bins. The bins grow geometrically, so equal-width bars draw a
 * logarithmic size axis.
 */
import { useMemo } from "react";

import { fmt } from "../format";
import { categoryAxis, valueAxis } from "./axes";
import { Chart, baseOption, tooltipStyle, type ChartOption } from "./chart";
import { DataTable } from "./table";
import { useChartTheme, type ChartTheme } from "./theme";
import { tooltipContent } from "./tooltip";

/** Items whose size is in `[lower, upper)`; `upper` is `null` for an open-ended bin. */
export interface HistogramBin {
  lower: number;
  upper: number | null;
  count: number;
}

/** How a bin bound is written; sizes by default, e.g. "1.2K". */
export type BoundFormat = (bound: number) => string;

const compactBound: BoundFormat = (bound) => fmt.compact(bound);

/** A bin's label, e.g. "1.0K–1.2K" or "4.3B+". */
export function binLabel(bin: HistogramBin, format: BoundFormat = compactBound): string {
  return bin.upper === null ? `${format(bin.lower)}+` : `${format(bin.lower)}–${format(bin.upper)}`;
}

interface ItemParam {
  dataIndex?: number;
}

function histogramOption(
  theme: ChartTheme,
  bins: readonly HistogramBin[],
  counted: string,
  bound: BoundFormat,
  unit: string,
): ChartOption {
  const total = bins.reduce((sum, b) => sum + b.count, 0);
  const color = theme.data[0] ?? theme.accent;
  return {
    ...baseOption(theme),
    grid: { left: 4, right: 8, top: 12, bottom: 4, containLabel: true },
    xAxis: categoryAxis(
      theme,
      bins.map((b) => binLabel(b, bound)),
    ),
    yAxis: valueAxis(theme, (v) => fmt.compact(v)),
    tooltip: {
      ...tooltipStyle(theme),
      trigger: "item",
      formatter: (p: unknown) => {
        const bin = bins[(p as ItemParam).dataIndex ?? -1];
        if (bin === undefined) return "";
        return tooltipContent(`${binLabel(bin, bound)} ${unit}`.trim(), [
          { label: counted, value: fmt.integer(bin.count), color },
          { label: "Share", value: fmt.percent(total === 0 ? 0 : bin.count / total) },
        ]);
      },
    },
    series: [
      {
        type: "bar",
        data: bins.map((b) => b.count),
        barCategoryGap: "8%",
        itemStyle: { color, borderRadius: [2, 2, 0, 0] },
      },
    ],
  };
}

export function Histogram({
  bins,
  label,
  counted,
  bound = compactBound,
  unit = "tokens",
}: {
  bins: readonly HistogramBin[];
  label: string;
  counted: string;
  bound?: BoundFormat;
  /** What the bounds measure, after the range in tooltips. */
  unit?: string;
}) {
  const theme = useChartTheme();
  const option = useMemo(
    () => histogramOption(theme, bins, counted, bound, unit),
    [theme, bins, counted, bound, unit],
  );
  return <Chart option={option} label={label} height={240} />;
}

/** The table twin of {@link Histogram}. */
export function HistogramTable({
  bins,
  caption,
  counted,
  bound = compactBound,
  header = "Tokens",
}: {
  bins: readonly HistogramBin[];
  caption: string;
  counted: string;
  bound?: BoundFormat;
  header?: string;
}) {
  return (
    <DataTable
      caption={caption}
      rows={bins}
      rowKey={(b) => String(b.lower)}
      columns={[
        { header, cell: (b) => binLabel(b, bound) },
        { header: counted, numeric: true, cell: (b) => fmt.integer(b.count) },
      ]}
    />
  );
}
