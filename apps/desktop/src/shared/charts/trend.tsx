import { useMemo } from "react";

import { MISSING, fmt } from "../format";
import { dateAxis, valueAxis } from "./axes";
import { Chart, baseOption, legendStyle, tooltipStyle, type ChartOption } from "./chart";
import { DataTable } from "./table";
import { useChartTheme, type ChartTheme } from "./theme";
import { tooltipContent, type TooltipLine } from "./tooltip";

export interface TrendSeries {
  name: string;
  color: string;
  /** `null` where the value was not reported (drawn as a gap, never as zero). */
  values: readonly (number | null)[];
}

/** The series id of an overlay: a line drawn over the stack, not part of its total. */
const OVERLAY = "overlay";

/**
 * Where an overlay is scaled: on the stack's axis, or on its own axis at the right (for a
 * running total that would dwarf the bars).
 */
export type OverlayAxis = "shared" | "own";

interface AxisParam {
  axisValue?: string;
  color?: string;
  seriesId?: string;
  seriesName?: string;
  value?: number | null;
}

function tooltip(params: unknown, format: (v: number) => string): HTMLElement | string {
  const list = (Array.isArray(params) ? params : [params]) as AxisParam[];
  const first = list[0];
  if (first === undefined) return "";
  const stacked = list.filter((p) => p.seriesId !== OVERLAY);
  const total = stacked.reduce((sum, p) => sum + (p.value ?? 0), 0);
  const lines: TooltipLine[] = list
    .filter((p) => (p.value ?? 0) > 0)
    .map((p) => ({
      label: p.seriesName ?? "",
      value: format(p.value ?? 0),
      ...(p.color === undefined ? {} : { color: p.color }),
    }));
  if (stacked.length > 1) lines.push({ label: "Total", value: format(total) });
  return tooltipContent(fmt.day(first.axisValue ?? ""), lines);
}

function stackedSeries(s: TrendSeries, area: boolean) {
  return {
    name: s.name,
    type: area ? "line" : "bar",
    stack: "total",
    data: s.values,
    itemStyle: { color: s.color, borderRadius: area ? 0 : [2, 2, 0, 0] },
    ...(area
      ? { areaStyle: { opacity: 0.22 }, symbol: "none", lineStyle: { width: 1.5, color: s.color } }
      : { barMaxWidth: 28 }),
    emphasis: { focus: "series" },
  };
}

function overlaySeries(s: TrendSeries, axis: OverlayAxis) {
  return {
    id: OVERLAY,
    yAxisIndex: axis === "own" ? 1 : 0,
    name: s.name,
    type: "line",
    data: s.values,
    symbol: "none",
    itemStyle: { color: s.color },
    lineStyle: { width: 2, color: s.color, type: "dashed" },
    emphasis: { focus: "series" },
  };
}

function trendOption(
  theme: ChartTheme,
  dates: readonly string[],
  series: readonly TrendSeries[],
  format: (v: number) => string,
  overlay: TrendSeries | undefined,
  overlayAxis: OverlayAxis,
): ChartOption {
  const area = dates.length > 31;
  const named = series.length + (overlay === undefined ? 0 : 1);
  return {
    ...baseOption(theme),
    legend: named > 1 ? legendStyle(theme, "roundRect") : { show: false },
    grid: { left: 4, right: 8, top: named > 1 ? 32 : 12, bottom: 4, containLabel: true },
    xAxis: dateAxis(theme, dates),
    yAxis:
      overlay !== undefined && overlayAxis === "own"
        ? [
            valueAxis(theme, format),
            { ...valueAxis(theme, format), position: "right", splitLine: { show: false } },
          ]
        : valueAxis(theme, format),
    tooltip: {
      ...tooltipStyle(theme),
      trigger: "axis",
      formatter: (p: unknown) => tooltip(p, format),
    },
    series: [
      ...series.map((s) => stackedSeries(s, area)),
      ...(overlay === undefined ? [] : [overlaySeries(overlay, overlayAxis)]),
    ],
  };
}

/**
 * Stacked bars (≤ 31 days) or stacked area (longer) over local days, with an optional
 * dashed overlay line that is not part of the stack; click selects a day.
 */
export function StackedTrend({
  dates,
  series,
  format,
  label,
  overlay,
  overlayAxis = "shared",
  onSelectDate,
}: {
  dates: readonly string[];
  series: readonly TrendSeries[];
  format: (v: number) => string;
  label: string;
  overlay?: TrendSeries | undefined;
  overlayAxis?: OverlayAxis;
  onSelectDate?: (date: string) => void;
}) {
  const theme = useChartTheme();
  const option = useMemo(
    () => trendOption(theme, dates, series, format, overlay, overlayAxis),
    [theme, dates, series, format, overlay, overlayAxis],
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

/**
 * The table twin of {@link StackedTrend}: one row per day, one column per series, the
 * total of the stack, then the overlay.
 */
export function TrendTable({
  dates,
  series,
  format,
  caption,
  overlay,
}: {
  dates: readonly string[];
  series: readonly TrendSeries[];
  format: (v: number) => string;
  caption: string;
  overlay?: TrendSeries | undefined;
}) {
  const rows = dates.map((date, i) => ({
    date,
    values: series.map((s) => s.values[i] ?? 0),
    overlay: overlay?.values[i] ?? null,
  }));
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
          cell: (r: Row) => format(r.values[i] ?? 0),
        })),
        ...(series.length > 1
          ? [
              {
                header: "Total",
                numeric: true,
                cell: (r: Row) => format(r.values.reduce((a, b) => a + b, 0)),
              },
            ]
          : []),
        ...(overlay === undefined
          ? []
          : [
              {
                header: overlay.name,
                numeric: true,
                cell: (r: Row) => (r.overlay === null ? MISSING : format(r.overlay)),
              },
            ]),
      ]}
    />
  );
}
