/**
 * Filter choices and chip labels, derived from what the core returned for the period
 * (so only values that occur are offered). Pure.
 */
import type { Labelled, Overview } from "@veyra/contracts";

import type { DimensionKey, FilterState } from "@/shared/filters";
import { fmt } from "@/shared/format";

export interface Option {
  value: string;
  label: string;
}

export type FilterOptions = Record<DimensionKey, Option[]>;

const byLabel = (a: Option, b: Option) => a.label.localeCompare(b.label);

/** Options from the core's labelled values. */
const fromLabelled = (values: readonly Labelled[] | undefined): Option[] =>
  (values ?? []).map((v) => ({ value: v.key, label: v.label })).sort(byLabel);

export function filterOptions(overview: Overview | undefined): FilterOptions {
  const models = (overview?.models ?? []).filter((r) => !r.isOther && r.key !== null);
  const projects = (overview?.projects ?? []).filter((r) => !r.isOther && r.key !== null);
  return {
    providers: fromLabelled(overview?.providers),
    models: models.map((m) => ({ value: m.key ?? "", label: m.label })).sort(byLabel),
    apps: fromLabelled(overview?.apps),
    interfaces: [],
    projects: projects
      .map((p) => ({ value: p.key ?? "", label: fmt.basename(p.label) }))
      .sort(byLabel),
  };
}

export interface ActiveFilter {
  dimension: DimensionKey;
  value: string;
  label: string;
}

/** Every active filter value with a readable label (falling back to the raw value). */
export function activeFilters(state: FilterState, options: FilterOptions): ActiveFilter[] {
  return (Object.keys(state.dimensions) as DimensionKey[]).flatMap((dimension) =>
    state.dimensions[dimension].map((value) => ({
      dimension,
      value,
      label: options[dimension].find((o) => o.value === value)?.label ?? value,
    })),
  );
}
