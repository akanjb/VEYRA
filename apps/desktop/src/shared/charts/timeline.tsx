import { useMemo } from "react";

import { fmt } from "../format";
import { Chart, baseOption, legendStyle, tooltipStyle, type ChartOption } from "./chart";
import { escapeHtml, useChartTheme, type ChartTheme } from "./theme";

export interface TimelinePoint {
  at: number;
  tokens: number;
  /** Series (usually the model). */
  group: string;
  color: string;
  detail: string;
}

/** Marker size grows with the square root of tokens, so area tracks the value. */
const MIN_SYMBOL = 5;
const SYMBOL_RANGE = 15;

function pointTooltip(theme: ChartTheme, timeZone: string | undefined) {
  return {
    ...tooltipStyle(theme),
    trigger: "item",
    formatter: (p: unknown) => {
      const point = (p as { data?: { point?: TimelinePoint } }).data?.point;
      if (point === undefined) return "";
      return `<b>${escapeHtml(fmt.time(point.at, timeZone))}</b> · ${escapeHtml(point.group)}<br/>${escapeHtml(point.detail)}`;
    },
  };
}

/** One scatter series per group (model), so the legend can isolate a model. */
export function timelineOption(
  theme: ChartTheme,
  points: readonly TimelinePoint[],
  timeZone?: string,
): ChartOption {
  const groups = [...new Map(points.map((p) => [p.group, p.color])).entries()];
  const max = Math.max(1, ...points.map((p) => p.tokens));
  return {
    ...baseOption(theme),
    legend: legendStyle(theme, "circle"),
    grid: { left: 4, right: 16, top: 32, bottom: 4, containLabel: true },
    xAxis: {
      type: "time",
      axisLine: { lineStyle: { color: theme.border } },
      axisLabel: {
        color: theme.subtle,
        formatter: (v: number) => fmt.time(v, timeZone).slice(0, 5),
      },
      splitLine: { show: false },
    },
    yAxis: {
      type: "value",
      name: "tokens",
      nameTextStyle: { color: theme.subtle },
      splitLine: { lineStyle: { color: theme.border } },
      axisLabel: { color: theme.subtle, formatter: (v: number) => fmt.compact(v) },
    },
    tooltip: pointTooltip(theme, timeZone),
    series: groups.map(([group, color]) => ({
      name: group,
      type: "scatter",
      itemStyle: { color, opacity: 0.85 },
      data: points
        .filter((p) => p.group === group)
        .map((p) => ({
          value: [p.at, p.tokens],
          symbolSize: MIN_SYMBOL + SYMBOL_RANGE * Math.sqrt(p.tokens / max),
          point: p,
        })),
    })),
  };
}

/**
 * Requests over time: one marker per request, sized by tokens and coloured by model, so
 * model switches show as colour changes (docs/planning/03-ux.md §4, Session detail).
 */
export function Timeline({
  points,
  label,
  timeZone,
}: {
  points: readonly TimelinePoint[];
  label: string;
  timeZone?: string;
}) {
  const theme = useChartTheme();
  const option = useMemo(() => timelineOption(theme, points, timeZone), [theme, points, timeZone]);
  return <Chart option={option} label={label} height={260} />;
}
