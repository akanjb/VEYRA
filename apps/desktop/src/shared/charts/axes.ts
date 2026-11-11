/** Axis options shared by the charts, so every chart's axes read the same way. */
import type { ChartTheme } from "./theme";

/** A category axis of local days (`YYYY-MM-DD`), labelled `MM/DD`. */
export function dateAxis(theme: ChartTheme, dates: readonly string[]) {
  return {
    type: "category",
    data: dates,
    axisLine: { lineStyle: { color: theme.border } },
    axisTick: { show: false },
    axisLabel: { color: theme.subtle, formatter: (v: string) => v.slice(5).replace("-", "/") },
  };
}

/** A category axis of arbitrary labels. */
export function categoryAxis(theme: ChartTheme, labels: readonly string[]) {
  return {
    type: "category",
    data: labels,
    axisLine: { lineStyle: { color: theme.border } },
    axisTick: { show: false },
    axisLabel: { color: theme.subtle, hideOverlap: true },
  };
}

/** A value axis with gridlines and formatted labels. */
export function valueAxis(theme: ChartTheme, format: (v: number) => string) {
  return {
    type: "value",
    splitLine: { lineStyle: { color: theme.border } },
    axisLabel: { color: theme.subtle, formatter: (v: number) => format(v) },
  };
}

/** A 0–100% axis on the right, for shares drawn over another measure. */
export function shareAxis(theme: ChartTheme, format: (v: number) => string) {
  return {
    type: "value",
    min: 0,
    max: 1,
    position: "right",
    splitLine: { show: false },
    axisLabel: { color: theme.subtle, formatter: (v: number) => format(v) },
  };
}
