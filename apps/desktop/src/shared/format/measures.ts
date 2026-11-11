/**
 * The chart measures (tokens, API-equivalent cost, requests): the single place that knows
 * how to label, read and format each one.
 */
import type { Measures } from "@veyra/contracts";

import type { Measure } from "../filters/filters";
import { compact, integer, usd } from "./format";

export interface MeasureMeta {
  label: string;
  /** Reads the measure from a group's measures. */
  pick: (measures: Measures) => number;
  /** Compact display, e.g. "12.4M" or "≈$4.4K". */
  format: (value: number | null) => string;
  /** Full precision, for tooltips and tables. */
  exact: (value: number | null) => string;
  /** Whether values are estimates (API-equivalent cost). */
  estimated: boolean;
}

export const MEASURE: Readonly<Record<Measure, MeasureMeta>> = {
  tokens: {
    label: "Tokens",
    pick: (m) => m.tokens,
    format: (v) => compact(v),
    exact: (v) => integer(v),
    estimated: false,
  },
  cost: {
    label: "Cost (API-equivalent)",
    pick: (m) => m.costNanoUsd,
    format: (v) => usd(v, { estimated: true, compact: true }),
    exact: (v) => usd(v, { estimated: true }),
    estimated: true,
  },
  requests: {
    label: "Requests",
    pick: (m) => m.requests,
    format: (v) => compact(v),
    exact: (v) => integer(v),
    estimated: false,
  },
};

/**
 * Money the user recorded (subscriptions and charges): exact amounts, so never marked as
 * estimates. A separate cost kind: never added to API-equivalent cost (ADR-004).
 */
export const RECORDED_MONEY = {
  format: (v: number | null) => usd(v, { compact: true }),
  exact: (v: number | null) => usd(v),
} as const;

/** A change in API-equivalent cost with its sign, e.g. "+≈$4.20" or "-≈$1.10". */
export function signedCost(v: number | null): string {
  if (v === null) return usd(v);
  const sign = v > 0 ? "+" : v < 0 ? "-" : "";
  return sign + usd(Math.abs(v), { estimated: true });
}
