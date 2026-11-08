import { useState } from "react";
import { ChevronDown, ChevronLeft, ChevronRight, SlidersHorizontal } from "lucide-react";
import type { Composition } from "@veyra/contracts";

import { useCommand } from "@/shared/api";
import {
  MEASURES,
  RANGES,
  clearDimensions,
  filterState,
  rangeLabel,
  stepRange,
  toScope,
  toggleValue,
  type FilterState,
  type Filters,
} from "@/shared/filters";
import { MEASURE } from "@/shared/format";
import { Button, Chip, CoverageMeter, Menu, MenuRadio } from "@/shared/ui";

import {
  activeFilters,
  filterOptions,
  type ActiveFilter,
  type FilterOptions,
} from "./filter-options";
import { FiltersSheet } from "./filters-sheet";

const MEASURE_OPTIONS = MEASURES.map((m) => ({ value: m, label: MEASURE[m].label }));
const RANGE_OPTIONS = RANGES.filter((r) => r !== "custom").map((r) => ({
  value: r,
  label: rangeLabel(filterState({ range: r })),
}));
const COMPARE_OPTIONS = [
  { value: "prev", label: "previous period" },
  { value: "none", label: "no comparison" },
] as const;

/** One editable word of the query sentence: shows the current choice, opens its menu. */
function TokenMenu<T extends string>({
  label,
  value,
  text,
  options,
  onChange,
}: {
  label: string;
  value: T;
  /** Shown instead of the option label (e.g. a custom period). */
  text?: string;
  options: readonly { value: T; label: string }[];
  onChange: (value: T) => void;
}) {
  const shown = text ?? options.find((o) => o.value === value)?.label ?? value;
  const trigger = (
    <Button size="sm" className="font-semibold">
      {shown}
      <ChevronDown size={14} aria-hidden />
    </Button>
  );
  return (
    <Menu label={`${label}: ${shown}`} trigger={trigger}>
      <MenuRadio value={value} options={options} onChange={onChange} />
    </Menu>
  );
}

function Stepper({
  state,
  today,
  onChange,
}: {
  state: FilterState;
  today: string;
  onChange: (next: FilterState) => void;
}) {
  const step = (direction: -1 | 1, label: string, Icon: typeof ChevronLeft) => (
    <Button
      size="sm"
      variant="ghost"
      aria-label={label}
      onClick={() => {
        onChange(stepRange(state, direction, today));
      }}
    >
      <Icon size={14} aria-hidden />
    </Button>
  );
  return (
    <>
      {step(-1, "Previous period (,)", ChevronLeft)}
      {step(1, "Next period (.)", ChevronRight)}
    </>
  );
}

function Sentence({
  filters,
  onOpenFilters,
  filterCount,
}: {
  filters: Filters;
  onOpenFilters: () => void;
  filterCount: number;
}) {
  const { state, setState, today } = filters;
  return (
    <div className="flex flex-wrap items-center gap-2 text-sm">
      <span className="text-muted">Showing</span>
      <TokenMenu
        label="Measure"
        value={state.measure}
        options={MEASURE_OPTIONS}
        onChange={(measure) => {
          setState({ ...state, measure });
        }}
      />
      <span className="text-muted">for</span>
      <TokenMenu
        label="Period"
        value={state.range}
        text={rangeLabel(state)}
        options={RANGE_OPTIONS}
        onChange={(range) => {
          setState({ ...state, range, from: null, to: null });
        }}
      />
      <Stepper state={state} today={today} onChange={setState} />
      <span className="text-muted">vs</span>
      <TokenMenu
        label="Comparison"
        value={state.compare ? "prev" : "none"}
        options={COMPARE_OPTIONS}
        onChange={(v) => {
          setState({ ...state, compare: v === "prev" });
        }}
      />
      <Button size="sm" className="ml-auto" onClick={onOpenFilters}>
        <SlidersHorizontal size={14} aria-hidden /> Filters
        {filterCount > 0 ? ` (${String(filterCount)})` : ""}
      </Button>
    </div>
  );
}

function ChipRow({
  chips,
  onRemove,
  composition,
}: {
  chips: readonly ActiveFilter[];
  onRemove: (chip: ActiveFilter) => void;
  composition: Composition | undefined;
}) {
  return (
    <div className="flex min-h-6 flex-wrap items-center gap-2">
      {chips.map((chip) => (
        <Chip
          key={`${chip.dimension}:${chip.value}`}
          label={`Remove filter ${chip.label}`}
          onRemove={() => {
            onRemove(chip);
          }}
        >
          {chip.label}
        </Chip>
      ))}
      <span className="ml-auto">
        <CoverageMeter composition={composition} />
      </span>
    </div>
  );
}

/** Filter choices from the unfiltered period, so filtering never hides alternatives. */
function useFilterOptions({ state, today, timeZone }: Filters): FilterOptions {
  const unfiltered = toScope(clearDimensions(state), today, timeZone);
  return filterOptions(useCommand("get_overview", { scope: unfiltered }).data);
}

/** "Showing [measure] for [period] ‹ › vs [comparison]" plus filters and coverage (§2.2). */
export function QueryBar({ filters }: { filters: Filters }) {
  const { state, setState } = filters;
  const [open, setOpen] = useState(false);
  const options = useFilterOptions(filters);
  const composition = useCommand("get_overview", { scope: filters.scope }).data?.composition;
  const chips = activeFilters(state, options);
  return (
    <div className="grid gap-2 border-b border-border bg-surface px-6 py-3">
      <Sentence
        filters={filters}
        filterCount={chips.length}
        onOpenFilters={() => {
          setOpen(true);
        }}
      />
      <ChipRow
        chips={chips}
        composition={composition}
        onRemove={(chip) => {
          setState(toggleValue(state, chip.dimension, chip.value));
        }}
      />
      <FiltersSheet
        open={open}
        onOpenChange={setOpen}
        state={state}
        options={options}
        onChange={setState}
      />
    </div>
  );
}
