/**
 * The global filter model (docs/planning/03-ux.md §2.2). The URL is the single source of
 * truth (ADR-013): search params are validated here, unknown or invalid values fall back
 * to defaults, and the result converts to the `Scope` every analytics command takes.
 */
import type { Comparison, Scope } from "@veyra/contracts";

import { addDays, daysInclusive, isIsoDate, monthStart, yearStart } from "./dates";

export const RANGES = ["today", "7d", "30d", "90d", "mtd", "ytd", "custom"] as const;
export type RangePreset = (typeof RANGES)[number];

export const MEASURES = ["tokens", "cost", "requests"] as const;
export type Measure = (typeof MEASURES)[number];

/** Filterable dimensions, in display order, with their URL parameter names. */
export const DIMENSIONS = [
  { key: "providers", param: "provider", label: "Provider" },
  { key: "models", param: "model", label: "Model" },
  { key: "apps", param: "app", label: "Application" },
  { key: "interfaces", param: "iface", label: "Interface" },
  { key: "projects", param: "project", label: "Project" },
] as const;
export type DimensionKey = (typeof DIMENSIONS)[number]["key"];

/** Every search key owned by the global filter model (page-specific keys are separate). */
export const FILTER_KEYS = [
  "range",
  "from",
  "to",
  "cmp",
  "m",
  "provider",
  "model",
  "app",
  "iface",
  "project",
] as const;

/** Search params as they appear in the URL (all optional strings; lists comma-joined). */
export interface SearchParams {
  range?: string;
  from?: string;
  to?: string;
  cmp?: string;
  m?: string;
  provider?: string;
  model?: string;
  app?: string;
  iface?: string;
  project?: string;
}

/** The validated filter state. */
export interface FilterState {
  range: RangePreset;
  /** Custom range bounds (inclusive local days); set only when `range` is "custom". */
  from: string | null;
  to: string | null;
  compare: boolean;
  measure: Measure;
  dimensions: Record<DimensionKey, string[]>;
}

export const DEFAULT_RANGE: RangePreset = "30d";
const MAX_LIST = 50;

const oneOf = <T extends string>(options: readonly T[], value: unknown): T | undefined =>
  options.find((o) => o === value);

function list(value: unknown): string[] {
  if (typeof value !== "string" || value === "") return [];
  return [
    ...new Set(
      value
        .split(",")
        .map((v) => v.trim())
        .filter((v) => v !== ""),
    ),
  ].slice(0, MAX_LIST);
}

/**
 * Validates raw search params (TanStack Router `validateSearch`): keeps only known, valid
 * values so the URL stays canonical.
 */
export function validateSearch(raw: Record<string, unknown>): SearchParams {
  const search: SearchParams = {};
  const range = oneOf(RANGES, raw["range"]);
  const from = typeof raw["from"] === "string" && isIsoDate(raw["from"]) ? raw["from"] : undefined;
  const to = typeof raw["to"] === "string" && isIsoDate(raw["to"]) ? raw["to"] : undefined;
  if (range === "custom") {
    if (from !== undefined && to !== undefined && from <= to)
      Object.assign(search, { range, from, to });
  } else if (range !== undefined && range !== DEFAULT_RANGE) {
    search.range = range;
  }
  if (raw["cmp"] === "none") search.cmp = "none";
  const measure = oneOf(MEASURES, raw["m"]);
  if (measure !== undefined && measure !== "tokens") search.m = measure;
  for (const { param } of DIMENSIONS) {
    const values = list(raw[param]);
    if (values.length > 0) search[param] = values.join(",");
  }
  return search;
}

/** The filter state described by validated search params. */
export function filterState(search: SearchParams): FilterState {
  const custom = search.range === "custom" && search.from !== undefined && search.to !== undefined;
  const dimensions = Object.fromEntries(
    DIMENSIONS.map(({ key, param }) => [key, list(search[param])]),
  ) as Record<DimensionKey, string[]>;
  return {
    range: custom ? "custom" : (oneOf(RANGES, search.range) ?? DEFAULT_RANGE),
    from: custom ? (search.from ?? null) : null,
    to: custom ? (search.to ?? null) : null,
    compare: search.cmp !== "none",
    measure: oneOf(MEASURES, search.m) ?? "tokens",
    dimensions,
  };
}

/** Search params for a filter state (inverse of {@link filterState}). */
export function toSearch(state: FilterState): SearchParams {
  const raw: Record<string, unknown> = {
    range: state.range,
    from: state.from,
    to: state.to,
    cmp: state.compare ? undefined : "none",
    m: state.measure,
  };
  for (const { key, param } of DIMENSIONS) raw[param] = state.dimensions[key].join(",");
  return validateSearch(raw);
}

/** The inclusive local days a filter state covers, given today's date. */
export function rangeDates(state: FilterState, today: string): { start: string; end: string } {
  switch (state.range) {
    case "today":
      return { start: today, end: today };
    case "7d":
      return { start: addDays(today, -6), end: today };
    case "30d":
      return { start: addDays(today, -29), end: today };
    case "90d":
      return { start: addDays(today, -89), end: today };
    case "mtd":
      return { start: monthStart(today), end: today };
    case "ytd":
      return { start: yearStart(today), end: today };
    case "custom":
      return { start: state.from ?? today, end: state.to ?? today };
  }
}

/** Moves the period one length earlier (-1) or later (+1); the result is a custom range. */
export function stepRange(state: FilterState, direction: -1 | 1, today: string): FilterState {
  const { start, end } = rangeDates(state, today);
  const length = daysInclusive(start, end);
  return {
    ...state,
    range: "custom",
    from: addDays(start, direction * length),
    to: addDays(end, direction * length),
  };
}

/** The `Scope` the analytics commands take. */
export function toScope(state: FilterState, today: string, timeZone: string): Scope {
  const { start, end } = rangeDates(state, today);
  const comparison: Comparison = state.compare ? "previous" : "none";
  return { timeZone, start, end, comparison, filters: { ...state.dimensions } };
}

/** A short label for the period: "Last 30 days", "Nov 1 – Nov 7, 2026". */
export function rangeLabel(state: FilterState): string {
  switch (state.range) {
    case "today":
      return "Today";
    case "7d":
      return "Last 7 days";
    case "30d":
      return "Last 30 days";
    case "90d":
      return "Last 90 days";
    case "mtd":
      return "Month to date";
    case "ytd":
      return "Year to date";
    case "custom":
      return state.from === state.to
        ? (state.from ?? "")
        : `${state.from ?? ""} – ${state.to ?? ""}`;
  }
}

/** Narrows the period to one local day (drill-down from a chart). */
export function withDay(state: FilterState, date: string): FilterState {
  return { ...state, range: "custom", from: date, to: date };
}

/** Adds `value` to a dimension's filter, or removes it when present. */
export function toggleValue(
  state: FilterState,
  dimension: DimensionKey,
  value: string,
): FilterState {
  const current = state.dimensions[dimension];
  const next = current.includes(value) ? current.filter((v) => v !== value) : [...current, value];
  return { ...state, dimensions: { ...state.dimensions, [dimension]: next } };
}

/** The state without any dimension filter. */
export function clearDimensions(state: FilterState): FilterState {
  return { ...state, dimensions: filterState({}).dimensions };
}
