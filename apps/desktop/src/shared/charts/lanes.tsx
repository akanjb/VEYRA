/**
 * Swimlanes: one row per item (such as a model), one cell per local day, shaded by the
 * day's value. Shows when each item was used and how heavily.
 */
import { useMemo } from "react";

import { MISSING, fmt } from "../format";
import { categoryAxis, dateAxis } from "./axes";
import { Chart, baseOption, tooltipStyle, type ChartOption } from "./chart";
import { DataTable } from "./table";
import { useChartTheme, type ChartTheme } from "./theme";
import { tooltipContent } from "./tooltip";

export interface Lane {
  label: string;
  /** One value per date. */
  values: readonly number[];
}

interface ValueParam {
  value?: unknown;
}

function lanesOption(
  theme: ChartTheme,
  dates: readonly string[],
  lanes: readonly Lane[],
  format: (v: number) => string,
): ChartOption {
  // Bottom-up: reverse so the first lane is drawn on top.
  const ordered = [...lanes].reverse();
  const cells = ordered.flatMap((lane, row) =>
    lane.values.flatMap((value, day) => (value > 0 ? [[day, row, value]] : [])),
  );
  return {
    ...baseOption(theme),
    grid: { left: 4, right: 8, top: 8, bottom: 4, containLabel: true },
    xAxis: { ...dateAxis(theme, dates), splitArea: { show: false } },
    yAxis: {
      ...categoryAxis(
        theme,
        ordered.map((l) => l.label),
      ),
      axisLabel: { color: theme.text, width: 140, overflow: "truncate" },
    },
    tooltip: {
      ...tooltipStyle(theme),
      formatter: (p: unknown) => {
        const [day, row, value] = ((p as ValueParam).value ?? [0, 0, 0]) as [
          number,
          number,
          number,
        ];
        return tooltipContent(ordered[row]?.label ?? "", [
          { label: fmt.day(dates[day] ?? ""), value: format(value) },
        ]);
      },
    },
    visualMap: {
      show: false,
      min: 0,
      max: Math.max(1, ...cells.map((c) => c[2] ?? 0)),
      inRange: { color: theme.seq.slice(2) },
    },
    series: [
      {
        type: "heatmap",
        data: cells,
        itemStyle: { borderColor: theme.surface, borderWidth: 1, borderRadius: 2 },
      },
    ],
  };
}

export function Lanes({
  dates,
  lanes,
  format,
  label,
}: {
  dates: readonly string[];
  lanes: readonly Lane[];
  format: (v: number) => string;
  label: string;
}) {
  const theme = useChartTheme();
  const option = useMemo(
    () => lanesOption(theme, dates, lanes, format),
    [theme, dates, lanes, format],
  );
  return <Chart option={option} label={label} height={Math.max(140, lanes.length * 26 + 40)} />;
}

/** The table twin of {@link Lanes}: each lane's active days, first and last day, total. */
export function LanesTable({
  dates,
  lanes,
  format,
  caption,
  header,
}: {
  dates: readonly string[];
  lanes: readonly Lane[];
  format: (v: number) => string;
  caption: string;
  header: string;
}) {
  const rows = lanes.map((lane) => {
    const active = lane.values.flatMap((v, i) => (v > 0 ? [i] : []));
    return {
      lane,
      days: active.length,
      first: dates[active[0] ?? -1],
      last: dates[active.at(-1) ?? -1],
      total: lane.values.reduce((a, b) => a + b, 0),
    };
  });
  const day = (date: string | undefined) => (date === undefined ? MISSING : fmt.day(date));
  return (
    <DataTable
      caption={caption}
      rows={rows}
      rowKey={(r, i) => `${r.lane.label}-${String(i)}`}
      columns={[
        { header, cell: (r) => r.lane.label },
        { header: "Days used", numeric: true, cell: (r) => fmt.integer(r.days) },
        { header: "First", cell: (r) => day(r.first) },
        { header: "Last", cell: (r) => day(r.last) },
        { header: "Total", numeric: true, cell: (r) => format(r.total) },
      ]}
    />
  );
}
