/**
 * Report previews over the synthetic dataset, shaped like the core's `preview_export`
 * (same columns, presets and redaction). The core builds real reports.
 */
import type { ExportPreset, ExportPreview, ExportRequest } from "@veyra/contracts";

import { localDay, select, tokensOf, type Flat } from "./aggregate";
import { FIXTURE_PROJECTS, fixtureLabel, type FixtureEvent } from "./dataset";

const PREVIEW_ROWS = 20;

const MEASURES = [
  "Input tokens",
  "Cache reads",
  "Cache writes",
  "Output tokens",
  "Reasoning tokens",
  "Total tokens",
  "Requests",
  "Cost (USD)",
  "Cost kind",
  "Token accuracy",
];

const NAMES: Record<ExportPreset, string> = {
  usageSummary: "usage-summary",
  costSummary: "cost-summary",
  byProject: "per-project",
  byModel: "per-model",
  daily: "daily",
  weekly: "weekly",
  monthly: "monthly",
};

function dollars(nano: number): string {
  return (nano / 1e9).toFixed(9);
}

function measured(keys: string[], events: readonly FixtureEvent[]): string[] {
  const sum = (pick: (e: FixtureEvent) => number) => events.reduce((n, e) => n + pick(e), 0);
  return [
    ...keys,
    String(sum((e) => e.input + e.cacheRead + e.cacheWrite)),
    String(sum((e) => e.cacheRead)),
    String(sum((e) => e.cacheWrite)),
    String(sum((e) => e.output)),
    String(sum((e) => e.reasoning)),
    String(sum(tokensOf)),
    String(events.length),
    dollars(sum((e) => e.costNanoUsd)),
    "API-equivalent (estimate)",
    "Exact",
  ];
}

/** Groups events by a key, largest by tokens first. */
function grouped(selected: readonly Flat[], keyOf: (f: Flat) => string[]): string[][] {
  const groups = new Map<string, { keys: string[]; events: FixtureEvent[] }>();
  for (const f of selected) {
    const keys = keyOf(f);
    const id = keys.join("\u0000");
    const group = groups.get(id) ?? { keys, events: [] };
    group.events.push(f.event);
    groups.set(id, group);
  }
  return [...groups.values()]
    .sort(
      (a, b) =>
        b.events.reduce((n, e) => n + tokensOf(e), 0) -
        a.events.reduce((n, e) => n + tokensOf(e), 0),
    )
    .map((g) => measured(g.keys, g.events));
}

/** A short stand-in for the core's salted hash. */
function pseudonym(path: string): string {
  let hash = 0;
  for (const char of path) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  return `project-${hash.toString(16).padStart(12, "0").slice(-12)}`;
}

export function exportPreview(flat: readonly Flat[], request: ExportRequest): ExportPreview {
  const { scope, preset, redaction } = request;
  const selected = select(flat, scope, scope.start, scope.end);
  const withPaths = !(redaction.dropPaths || redaction.hashProjectNames);
  let keys: string[];
  let rows: string[][];
  switch (preset) {
    case "byProject":
      keys = withPaths ? ["Project", "Path"] : ["Project"];
      rows = grouped(selected, ({ session }) => {
        const path = FIXTURE_PROJECTS[session.projectId - 1] ?? "";
        const name = redaction.hashProjectNames ? pseudonym(path) : path;
        return withPaths ? [name, path] : [name];
      });
      break;
    case "byModel":
      keys = ["Provider", "Model"];
      rows = grouped(selected, ({ event }) => [
        fixtureLabel(event.provider).label,
        fixtureLabel(event.model).label,
      ]);
      break;
    case "usageSummary":
      keys = ["Application"];
      rows = grouped(selected, ({ session }) => [fixtureLabel(session.app).label]);
      break;
    case "costSummary":
      keys = ["Provider or item"];
      rows = grouped(selected, ({ event }) => [fixtureLabel(event.provider).label]);
      break;
    case "daily":
    case "weekly":
    case "monthly": {
      keys = ["Period start"];
      const period = (day: string) =>
        preset === "monthly" ? `${day.slice(0, 8)}01` : preset === "weekly" ? weekStart(day) : day;
      rows = grouped(selected, ({ event }) => [period(localDay(event.at, scope.timeZone))]).sort(
        (a, b) => (a[0] ?? "").localeCompare(b[0] ?? ""),
      );
      break;
    }
  }
  const extension = request.format === "json" ? "json" : "csv";
  return {
    columns: [...keys, ...MEASURES],
    rows: rows.slice(0, PREVIEW_ROWS),
    rowCount: rows.length,
    fileName: `veyra-${NAMES[preset]}-${scope.start}-to-${scope.end}.${extension}`,
  };
}

/** The Monday on or before a `YYYY-MM-DD` day. */
function weekStart(day: string): string {
  const date = new Date(`${day}T00:00:00Z`);
  const back = (date.getUTCDay() + 6) % 7;
  date.setUTCDate(date.getUTCDate() - back);
  return date.toISOString().slice(0, 10);
}
