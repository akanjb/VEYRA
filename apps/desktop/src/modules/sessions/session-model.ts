/** Maps a session detail to chart and list inputs. Pure. */
import type { Labelled, SessionDetail, TimelineEvent } from "@veyra/contracts";

import { modelColors, type ChartTheme, type TimelinePoint } from "@/shared/charts";
import { fmt } from "@/shared/format";

const UNKNOWN_MODEL = { key: "unknown", label: "Unknown model" } as const;

/** One point per request, coloured by model within its provider's hue. */
export function timelinePoints(detail: SessionDetail, theme: ChartTheme): TimelinePoint[] {
  const colors = modelColors(
    theme,
    detail.events.map((e) => ({ id: (e.model ?? UNKNOWN_MODEL).key, provider: e.provider })),
  );
  return detail.events.map((e) => {
    const model = e.model ?? UNKNOWN_MODEL;
    return {
      at: e.at,
      tokens: e.tokens.total,
      group: model.label,
      color: colors.get(model.key) ?? theme.subtle,
      detail: describe(e),
    };
  });
}

/** Model names of a session, for display. */
export function modelNames(models: readonly Labelled[]): string {
  return models.map((m) => m.label).join(", ");
}

function describe(event: TimelineEvent): string {
  const cost =
    event.costNanoUsd === null ? "unpriced" : fmt.usd(event.costNanoUsd, { estimated: true });
  const tools =
    event.tools.length === 0 ? "" : ` · tools: ${event.tools.map((t) => t.name).join(", ")}`;
  return `${fmt.integer(event.tokens.total)} tokens · ${cost}${tools}`;
}

export interface ToolCount {
  name: string;
  mcpServer: string | null;
  calls: number;
  errors: number;
}

/** Tool calls of the session by name, most used first. */
export function toolCounts(events: readonly TimelineEvent[]): ToolCount[] {
  const counts = new Map<string, ToolCount>();
  for (const tool of events.flatMap((e) => e.tools)) {
    const entry = counts.get(tool.name) ?? {
      name: tool.name,
      mcpServer: tool.mcpServer,
      calls: 0,
      errors: 0,
    };
    entry.calls += 1;
    if (tool.isError === true) entry.errors += 1;
    counts.set(tool.name, entry);
  }
  return [...counts.values()].sort((a, b) => b.calls - a.calls || a.name.localeCompare(b.name));
}
