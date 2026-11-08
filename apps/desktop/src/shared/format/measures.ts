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
