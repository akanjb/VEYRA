import type { Kpi } from "@veyra/contracts";
import { ArrowDownRight, ArrowUpRight, Minus } from "lucide-react";

import { fmt } from "../format";
import { AccuracyBadge, type AccuracyLabel } from "./accuracy";
import { Tip, cn } from "./primitives";

/**
 * A relative change. Neutral colours on purpose: more usage is neither good nor bad
 * (semantic colours are for states only).
 */
export function DeltaChip({ kpi, format }: { kpi: Kpi; format: (value: number | null) => string }) {
  if (kpi.change === null) {
    const reason = kpi.previous === null ? "No comparison data" : "Previous period was zero";
    return <span className="text-xs text-subtle">{kpi.previous === null ? "" : reason}</span>;
  }
  const Icon = kpi.change > 0 ? ArrowUpRight : kpi.change < 0 ? ArrowDownRight : Minus;
  return (
    <Tip content={`Previous period: ${format(kpi.previous)}`}>
      <span
        tabIndex={0}
        className="inline-flex items-center gap-0.5 rounded-full bg-gray-3 px-1.5 py-px text-xs text-muted"
        aria-label={`${fmt.change(kpi.change)} versus the previous period (${format(kpi.previous)})`}
      >
        <Icon size={12} aria-hidden />
        <span data-numeric>{fmt.change(kpi.change)}</span>
      </span>
    </Tip>
  );
}

/** A tiny trend line; decorative (the card's table twin carries the values). */
export function Sparkline({
  values,
  className,
}: {
  values: readonly number[];
  className?: string;
}) {
  if (values.length < 2) return null;
  const max = Math.max(...values, 1);
  const step = 100 / (values.length - 1);
  const points = values.map(
    (v, i) => `${(i * step).toFixed(2)},${(28 - (v / max) * 26).toFixed(2)}`,
  );
  return (
    <svg
      viewBox="0 0 100 30"
      preserveAspectRatio="none"
      aria-hidden
      className={cn("h-8 w-full", className)}
    >
      <polyline
        points={points.join(" ")}
        fill="none"
        stroke="var(--accent)"
        strokeWidth="1.5"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}

interface KpiTileProps {
  label: string;
  kpi: Kpi;
  format: (value: number | null) => string;
  /** Full-precision value shown on hover. */
  exact?: (value: number | null) => string;
  accuracy?: AccuracyLabel | null;
  hero?: boolean;
  trend?: readonly number[];
}

export function KpiTile({
  label,
  kpi,
  format,
  exact,
  accuracy,
  hero = false,
  trend,
}: KpiTileProps) {
  const value = format(kpi.value);
  return (
    <div
      className={cn(
        "flex min-w-0 flex-col gap-1 rounded-card border border-border bg-surface p-4",
        hero && "col-span-2 md:col-span-3 xl:col-span-1",
      )}
    >
      <Tip content={kpi.definition}>
        <span tabIndex={0} className="truncate text-xs font-medium text-muted">
          {label}
        </span>
      </Tip>
      <Tip content={exact === undefined ? value : exact(kpi.value)}>
        <span
          tabIndex={0}
          data-numeric
          className={cn("font-semibold tracking-tight", hero ? "text-3xl" : "text-xl")}
        >
          {value}
        </span>
      </Tip>
      <div className="flex flex-wrap items-center gap-1.5">
        <DeltaChip kpi={kpi} format={format} />
        <AccuracyBadge kind={accuracy === undefined ? kpi.accuracy : accuracy} />
      </div>
      {trend === undefined ? null : <Sparkline values={trend} className="mt-1" />}
    </div>
  );
}
