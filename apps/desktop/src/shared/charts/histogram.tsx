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

/** A bin's label, e.g. "1.0K–1.2K" or "4.3B+". */
export function binLabel(bin: HistogramBin): string {
  return bin.upper === null
    ? `${fmt.compact(bin.lower)}+`
    : `${fmt.compact(bin.lower)}–${fmt.compact(bin.upper)}`;
}

interface ItemParam {
  dataIndex?: number;
}

function histogramOption(
  theme: ChartTheme,
  bins: readonly HistogramBin[],
  counted: string,
): ChartOption {
  const total = bins.reduce((sum, b) => sum + b.count, 0);
  const color = theme.data[0] ?? theme.accent;
  return {
    ...baseOption(theme),
    grid: { left: 4, right: 8, top: 12, bottom: 4, containLabel: true },
    xAxis: categoryAxis(theme, bins.map(binLabel)),
    yAxis: valueAxis(theme, (v) => fmt.compact(v)),
    tooltip: {
      ...tooltipStyle(theme),
      trigger: "item",
      formatter: (p: unknown) => {
        const bin = bins[(p as ItemParam).dataIndex ?? -1];
        if (bin === undefined) return "";
        return tooltipContent(`${binLabel(bin)} tokens`, [
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
}: {
  bins: readonly HistogramBin[];
  label: string;
  counted: string;
}) {
  const theme = useChartTheme();
  const option = useMemo(() => histogramOption(theme, bins, counted), [theme, bins, counted]);
  return <Chart option={option} label={label} height={240} />;
}

/** The table twin of {@link Histogram}. */
export function HistogramTable({
  bins,
  caption,
  counted,
}: {
  bins: readonly HistogramBin[];
  caption: string;
  counted: string;
}) {
  return (
    <DataTable
      caption={caption}
      rows={bins}
      rowKey={(b) => String(b.lower)}
      columns={[
        { header: "Tokens", cell: (b) => binLabel(b) },
        { header: counted, numeric: true, cell: (b) => fmt.integer(b.count) },
      ]}
    />
  );
}
