/** The single ECharts host: tree-shaken runtime, canvas rendering, resize and dispose. */
import { useEffect, useRef } from "react";
import { BarChart, HeatmapChart, LineChart, ScatterChart, TreemapChart } from "echarts/charts";
import {
  CalendarComponent,
  GridComponent,
  LegendComponent,
  MarkLineComponent,
  TooltipComponent,
  VisualMapComponent,
} from "echarts/components";
import * as echarts from "echarts/core";
import { CanvasRenderer } from "echarts/renderers";

import type { ChartTheme } from "./theme";

echarts.use([
  BarChart,
  HeatmapChart,
  LineChart,
  ScatterChart,
  TreemapChart,
  CalendarComponent,
  GridComponent,
  LegendComponent,
  MarkLineComponent,
  TooltipComponent,
  VisualMapComponent,
  CanvasRenderer,
]);

export type ChartOption = echarts.EChartsCoreOption;

/** Parameters ECharts passes to click handlers (the fields Veyra uses). */
export interface ChartClick {
  name?: string;
  dataIndex?: number;
  value?: unknown;
  data?: unknown;
}

/** Tooltip styling from the tokens; every chart spreads this, then adds its formatter. */
export function tooltipStyle(theme: ChartTheme) {
  return {
    backgroundColor: theme.raised,
    borderColor: theme.border,
    borderWidth: 1,
    padding: [8, 10],
    textStyle: { color: theme.text, fontSize: 12, fontFamily: theme.font },
    confine: true,
  };
}

/** Legends sit at the top left and toggle series on click (§5.2). */
export function legendStyle(theme: ChartTheme, icon: "roundRect" | "circle") {
  const size = icon === "circle" ? 8 : 10;
  return {
    top: 0,
    left: 0,
    icon,
    itemWidth: size,
    itemHeight: size,
    textStyle: { color: theme.muted },
  };
}

/** Shared option defaults: no animation on data load, token-based text and tooltips. */
export function baseOption(theme: ChartTheme): ChartOption {
  return {
    animation: false,
    textStyle: { fontFamily: theme.font, color: theme.muted, fontSize: 12 },
    tooltip: tooltipStyle(theme),
  };
}

interface ChartProps {
  option: ChartOption;
  /** A one-sentence summary for assistive technology (the table twin has the values). */
  label: string;
  height?: number;
  onClick?: ((params: ChartClick) => void) | undefined;
}

export function Chart({ option, label, height = 260, onClick }: ChartProps) {
  const host = useRef<HTMLDivElement>(null);
  const instance = useRef<echarts.ECharts | null>(null);

  useEffect(() => {
    const element = host.current;
    if (element === null) return undefined;
    const chart = echarts.init(element, undefined, { renderer: "canvas" });
    instance.current = chart;
    const observer = new ResizeObserver(() => {
      chart.resize();
    });
    observer.observe(element);
    return () => {
      observer.disconnect();
      chart.dispose();
      instance.current = null;
    };
  }, []);

  useEffect(() => {
    instance.current?.setOption(option, { notMerge: true });
  }, [option]);

  useEffect(() => {
    const chart = instance.current;
    if (chart === null || onClick === undefined) return undefined;
    const handler = (params: unknown) => {
      onClick(params as ChartClick);
    };
    chart.on("click", handler);
    return () => {
      chart.off("click", handler);
    };
  }, [onClick]);

  return <div ref={host} role="img" aria-label={label} className="w-full" style={{ height }} />;
}
