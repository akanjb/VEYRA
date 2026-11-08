/** Source health in plain language, and its indicator: the single definition of both. */
import type { SourceSummary } from "@veyra/contracts";

import { fmt } from "@/shared/format";
import { cn } from "@/shared/ui";

export type HealthTone = "ok" | "warn" | "off";

export function health(source: SourceSummary): { label: string; tone: HealthTone } {
  if (!source.enabled) return { label: "Not enabled", tone: "off" };
  if (source.lastError !== null) return { label: "Last scan failed", tone: "warn" };
  if (source.lastSuccessAt === null) return { label: "Waiting for first scan", tone: "warn" };
  return { label: `Up to date · ${fmt.ago(source.lastSuccessAt)}`, tone: "ok" };
}

/** The health of several sources together: nothing enabled, any problem, or all well. */
export function overallHealth(sources: readonly SourceSummary[]): {
  label: string;
  tone: HealthTone;
} {
  const enabled = sources.filter((s) => s.enabled);
  if (enabled.length === 0) return { label: "No sources enabled", tone: "off" };
  const count = `${String(enabled.length)} ${enabled.length === 1 ? "source" : "sources"}`;
  return enabled.some((s) => health(s).tone === "warn")
    ? { label: `${count} · needs attention`, tone: "warn" }
    : { label: count, tone: "ok" };
}

const TONE_CLASS: Readonly<Record<HealthTone, string>> = {
  ok: "bg-success",
  warn: "bg-warning",
  off: "bg-gray-6",
};

/** A status dot; the surrounding text always states the tone in words. */
export function HealthDot({ tone }: { tone: HealthTone }) {
  return <span className={cn("size-2 shrink-0 rounded-full", TONE_CLASS[tone])} aria-hidden />;
}
