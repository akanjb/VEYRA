import { useMemo } from "react";

import { categoryAxis, valueAxis } from "./axes";
import { Chart, baseOption, tooltipStyle, type ChartOption } from "./chart";
import { DataTable } from "./table";
import { useChartTheme, type ChartTheme } from "./theme";
import { tooltipContent } from "./tooltip";

/** One bar of a waterfall: a total, or a change from the bar before. */
export interface WaterfallBar {
  label: string;
  /** For a total, its value; for a change, the amount added (negative removes). */
  value: number;
  kind: "total" | "change";
  color: string;
}

type Format = (v: number) => string;

/** Each bar's floating range: where it starts and where it ends. */
export function waterfallRanges(bars: readonly WaterfallBar[]): { low: number; high: number }[] {
  let running = 0;
  return bars.map((bar) => {
    if (bar.kind === "total") {
      running = bar.value;
      return { low: Math.min(0, bar.value), high: Math.max(0, bar.value) };
    }
    const start = running;
    running += bar.value;
    return { low: Math.min(start, running), high: Math.max(start, running) };
  });
}

function waterfallOption(
  theme: ChartTheme,
  bars: readonly WaterfallBar[],
  format: Format,
  signed: Format,
): ChartOption {
  const ranges = waterfallRanges(bars);
  return {
    ...baseOption(theme),
    grid: { left: 4, right: 8, top: 12, bottom: 4, containLabel: true },
    xAxis: categoryAxis(
      theme,
      bars.map((b) => b.label),
    ),
    yAxis: valueAxis(theme, format),
    tooltip: {
      ...tooltipStyle(theme),
      trigger: "item",
      formatter: (p: unknown) => {
        const bar = bars[(p as { dataIndex?: number }).dataIndex ?? -1];
        if (bar === undefined) return "";
        const value = bar.kind === "total" ? format(bar.value) : signed(bar.value);
        return tooltipContent(bar.label, [{ label: value, color: bar.color }]);
      },
    },
    series: [
      {
        // The invisible base each visible bar floats on.
        type: "bar",
        stack: "waterfall",
        silent: true,
        itemStyle: { color: "transparent" },
        data: ranges.map((r) => r.low),
      },
      {
        type: "bar",
        stack: "waterfall",
        barMaxWidth: 36,
        data: ranges.map((r, i) => ({
          value: r.high - r.low,
          itemStyle: { color: bars[i]?.color, borderRadius: 2 },
        })),
      },
    ],
  };
}

/** A waterfall: a starting total, the changes that lead from it, and the end total. */
export function WaterfallChart({
  bars,
  format,
  signed,
  label,
}: {
  bars: readonly WaterfallBar[];
  /** Formats totals and axis values. */
  format: Format;
  /** Formats changes with their sign. */
  signed: Format;
  label: string;
}) {
  const theme = useChartTheme();
  const option = useMemo(
    () => waterfallOption(theme, bars, format, signed),
    [theme, bars, format, signed],
  );
  return <Chart option={option} label={label} height={280} />;
}

/** The table twin of {@link WaterfallChart}. */
export function WaterfallTable({
  bars,
  format,
  signed,
  caption,
}: {
  bars: readonly WaterfallBar[];
  format: Format;
  signed: Format;
  caption: string;
}) {
  return (
    <DataTable
      caption={caption}
      rows={bars}
      rowKey={(b, i) => `${b.label}-${String(i)}`}
      columns={[
        { header: "Step", cell: (b) => <span className="break-all">{b.label}</span> },
        {
          header: "Amount",
          numeric: true,
          cell: (b) => (b.kind === "total" ? format(b.value) : signed(b.value)),
        },
      ]}
    />
  );
}
