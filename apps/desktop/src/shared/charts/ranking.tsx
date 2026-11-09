import { useMemo } from "react";

import { fmt } from "../format";
import { Chart, baseOption, tooltipStyle, type ChartClick, type ChartOption } from "./chart";
import { DataTable } from "./table";
import { readableOn } from "./colors";
import { useChartTheme, type ChartTheme } from "./theme";
import { tooltipContent } from "./tooltip";

export interface RankItem {
  /** Filter value; `null` for "Other" and unknown values (not selectable). */
  key: string | null;
  label: string;
  value: number;
  /** Share of the whole (0–1). */
  share: number;
  color: string;
}

type Format = (v: number) => string;

interface ItemParam {
  dataIndex?: number;
}

/** Tooltip of one ranked item: label, value and share. */
function itemTooltip(theme: ChartTheme, items: readonly RankItem[], format: Format) {
  return {
    ...tooltipStyle(theme),
    trigger: "item",
    formatter: (p: unknown) => {
      const item = items[(p as ItemParam).dataIndex ?? -1];
      if (item === undefined) return "";
      return tooltipContent(item.label, [
        { label: format(item.value), value: fmt.percent(item.share), color: item.color },
      ]);
    },
  };
}

/** A click handler selecting the clicked item's key (ignoring "Other" and unknowns). */
function selectByIndex(items: readonly RankItem[], onSelect: ((key: string) => void) | undefined) {
  if (onSelect === undefined) return undefined;
  return (p: ChartClick) => {
    const key = items[p.dataIndex ?? -1]?.key;
    if (key !== undefined && key !== null) onSelect(key);
  };
}

/** Bars drawn bottom-up, so the items are reversed to put the largest on top. */
export function rankingOption(
  theme: ChartTheme,
  ordered: readonly RankItem[],
  format: Format,
): ChartOption {
  return {
    ...baseOption(theme),
    grid: { left: 4, right: 56, top: 4, bottom: 4, containLabel: true },
    xAxis: { type: "value", show: false },
    yAxis: {
      type: "category",
      data: ordered.map((i) => i.label),
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: { color: theme.text, width: 160, overflow: "truncate" },
    },
    tooltip: itemTooltip(theme, ordered, format),
    series: [
      {
        type: "bar",
        data: ordered.map((i) => ({
          value: i.value,
          itemStyle: { color: i.color, borderRadius: [0, 3, 3, 0] },
        })),
        barMaxWidth: 18,
        label: {
          show: true,
          position: "right",
          color: theme.muted,
          formatter: (p: unknown) =>
            fmt.percent(ordered[(p as ItemParam).dataIndex ?? -1]?.share ?? 0),
        },
      },
    ],
  };
}

export function treemapOption(
  theme: ChartTheme,
  items: readonly RankItem[],
  format: Format,
): ChartOption {
  return {
    ...baseOption(theme),
    tooltip: itemTooltip(theme, items, format),
    series: [
      {
        type: "treemap",
        roam: false,
        nodeClick: false,
        breadcrumb: { show: false },
        width: "100%",
        height: "100%",
        itemStyle: { borderColor: theme.surface, borderWidth: 2, gapWidth: 2 },
        label: {
          fontSize: 12,
          formatter: (p: unknown) => fmt.basename((p as { name?: string }).name ?? ""),
        },
        data: items.map((i) => ({
          name: i.label,
          value: i.value,
          itemStyle: { color: i.color },
          label: { color: readableOn(i.color) },
        })),
      },
    ],
  };
}

interface RankChartProps {
  items: readonly RankItem[];
  format: Format;
  label: string;
  onSelect?: (key: string) => void;
}

/** Horizontal ranking bars with share, largest first; better read than a donut (§4). */
export function Ranking({ items, format, label, onSelect }: RankChartProps) {
  const theme = useChartTheme();
  const ordered = useMemo(() => [...items].reverse(), [items]);
  const option = useMemo(() => rankingOption(theme, ordered, format), [theme, ordered, format]);
  const height = Math.max(120, items.length * 30 + 16);
  return (
    <Chart
      option={option}
      label={label}
      height={height}
      onClick={selectByIndex(ordered, onSelect)}
    />
  );
}

/** Proportional areas for many groups (top 30 + Other). */
export function Treemap({ items, format, label, onSelect }: RankChartProps) {
  const theme = useChartTheme();
  const option = useMemo(() => treemapOption(theme, items, format), [theme, items, format]);
  return (
    <Chart option={option} label={label} height={280} onClick={selectByIndex(items, onSelect)} />
  );
}

/** The table twin of {@link Ranking} and {@link Treemap}. */
export function RankTable({
  items,
  format,
  caption,
  header,
}: {
  items: readonly RankItem[];
  format: Format;
  caption: string;
  header: string;
}) {
  return (
    <DataTable
      caption={caption}
      rows={items}
      rowKey={(r, i) => `${r.key ?? "none"}-${String(i)}`}
      columns={[
        { header, cell: (r) => <span className="break-all">{r.label}</span> },
        { header: "Value", numeric: true, cell: (r) => format(r.value) },
        { header: "Share", numeric: true, cell: (r) => fmt.percent(r.share) },
      ]}
    />
  );
}
