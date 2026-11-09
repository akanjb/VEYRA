import { useMemo } from "react";

import { fmt } from "../format";
import { Chart, baseOption, tooltipStyle, type ChartOption } from "./chart";
import { DataTable } from "./table";
import { useChartTheme, type ChartTheme } from "./theme";
import { tooltipContent } from "./tooltip";

export const WEEKDAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] as const;

export interface DayValue {
  date: string;
  value: number;
}

export interface HourValue {
  weekday: number;
  hour: number;
  value: number;
}

interface ValueParam {
  value?: unknown;
}

/** The calendar heatmap option: empty days use the lightest ramp step. */
export function calendarOption(
  theme: ChartTheme,
  days: readonly DayValue[],
  format: (v: number) => string,
): ChartOption {
  const max = Math.max(1, ...days.map((d) => d.value));
  return {
    ...baseOption(theme),
    tooltip: {
      ...tooltipStyle(theme),
      formatter: (p: unknown) => {
        const [date, value] = ((p as ValueParam).value ?? ["", 0]) as [string, number];
        return tooltipContent(fmt.day(date), [{ label: format(value) }]);
      },
    },
    visualMap: { show: false, min: 0, max, inRange: { color: theme.seq.slice(1) } },
    calendar: {
      range: [days[0]?.date ?? "", days.at(-1)?.date ?? ""],
      top: 22,
      left: 32,
      right: 8,
      cellSize: ["auto", 13],
      splitLine: { show: false },
      itemStyle: { color: theme.seq[0], borderColor: theme.surface, borderWidth: 2 },
      dayLabel: {
        firstDay: 1,
        color: theme.subtle,
        fontSize: 10,
        nameMap: ["S", "M", "T", "W", "T", "F", "S"],
      },
      monthLabel: { color: theme.subtle, fontSize: 10 },
      yearLabel: { show: false },
    },
    series: [
      {
        type: "heatmap",
        coordinateSystem: "calendar",
        data: days.filter((d) => d.value > 0).map((d) => [d.date, d.value]),
      },
    ],
  };
}

/** The weekday × hour heatmap option (rows Monday to Sunday). */
export function hourOption(
  theme: ChartTheme,
  cells: readonly HourValue[],
  format: (v: number) => string,
): ChartOption {
  const axis = { axisTick: { show: false }, axisLine: { show: false } };
  return {
    ...baseOption(theme),
    grid: { left: 4, right: 8, top: 4, bottom: 4, containLabel: true },
    xAxis: {
      ...axis,
      type: "category",
      data: Array.from({ length: 24 }, (_, h) => String(h).padStart(2, "0")),
      axisLabel: { color: theme.subtle, interval: 2 },
    },
    yAxis: {
      ...axis,
      type: "category",
      data: [...WEEKDAYS],
      inverse: true,
      axisLabel: { color: theme.subtle },
    },
    tooltip: {
      ...tooltipStyle(theme),
      formatter: (p: unknown) => {
        const [hour, weekday, value] = ((p as ValueParam).value ?? [0, 0, 0]) as [
          number,
          number,
          number,
        ];
        const slot = `${WEEKDAYS[weekday] ?? ""} ${String(hour).padStart(2, "0")}:00`;
        return tooltipContent(slot, [{ label: format(value) }]);
      },
    },
    visualMap: {
      show: false,
      min: 0,
      max: Math.max(1, ...cells.map((c) => c.value)),
      inRange: { color: theme.seq },
    },
    series: [
      {
        type: "heatmap",
        data: cells.map((c) => [c.hour, c.weekday, c.value]),
        itemStyle: { borderColor: theme.surface, borderWidth: 2, borderRadius: 2 },
      },
    ],
  };
}

/** The clicked calendar day, if the click landed on one. */
function clickedDate(p: ValueParam): string | undefined {
  return Array.isArray(p.value) && typeof p.value[0] === "string" ? p.value[0] : undefined;
}

/** A year of local days as a calendar heatmap; click selects a day. */
export function CalendarHeatmap({
  days,
  format,
  label,
  onSelectDate,
}: {
  days: readonly DayValue[];
  format: (v: number) => string;
  label: string;
  onSelectDate?: (date: string) => void;
}) {
  const theme = useChartTheme();
  const option = useMemo(() => calendarOption(theme, days, format), [theme, days, format]);
  const onClick =
    onSelectDate === undefined
      ? undefined
      : (p: ValueParam) => {
          const date = clickedDate(p);
          if (date !== undefined) onSelectDate(date);
        };
  return <Chart option={option} label={label} height={150} onClick={onClick} />;
}

/** Tokens by local weekday (rows) and hour (columns). */
export function HourHeatmap({
  cells,
  format,
  label,
}: {
  cells: readonly HourValue[];
  format: (v: number) => string;
  label: string;
}) {
  const theme = useChartTheme();
  const option = useMemo(() => hourOption(theme, cells, format), [theme, cells, format]);
  return <Chart option={option} label={label} height={220} />;
}

/** The table twin of {@link HourHeatmap}: busiest hours first. */
export function HourTable({
  cells,
  format,
  caption,
}: {
  cells: readonly HourValue[];
  format: (v: number) => string;
  caption: string;
}) {
  const rows = cells.filter((c) => c.value > 0).sort((a, b) => b.value - a.value);
  return (
    <DataTable
      caption={caption}
      rows={rows}
      rowKey={(r) => `${String(r.weekday)}-${String(r.hour)}`}
      columns={[
        { header: "Day", cell: (r) => WEEKDAYS[r.weekday] ?? "" },
        { header: "Hour", cell: (r) => `${String(r.hour).padStart(2, "0")}:00` },
        { header: "Value", numeric: true, cell: (r) => format(r.value) },
      ]}
    />
  );
}
