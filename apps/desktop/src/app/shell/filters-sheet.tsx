import { useState } from "react";

import { DIMENSIONS, clearDimensions, toggleValue, type FilterState } from "@/shared/filters";
import { Button, Sheet } from "@/shared/ui";

import type { FilterOptions } from "./filter-options";

function CustomPeriod({
  state,
  onChange,
}: {
  state: FilterState;
  onChange: (next: FilterState) => void;
}) {
  const [from, setFrom] = useState(state.from ?? "");
  const [to, setTo] = useState(state.to ?? "");
  const valid = from !== "" && to !== "" && from <= to;
  return (
    <fieldset className="grid gap-2">
      <legend className="mb-1 text-xs font-medium tracking-wide text-subtle uppercase">
        Custom period
      </legend>
      <div className="flex flex-wrap items-end gap-2">
        <label className="grid gap-1 text-xs text-muted">
          From
          <input
            type="date"
            value={from}
            onChange={(e) => {
              setFrom(e.target.value);
            }}
            className="h-8 rounded-control border border-border bg-surface px-2 text-sm text-text"
          />
        </label>
        <label className="grid gap-1 text-xs text-muted">
          To
          <input
            type="date"
            value={to}
            onChange={(e) => {
              setTo(e.target.value);
            }}
            className="h-8 rounded-control border border-border bg-surface px-2 text-sm text-text"
          />
        </label>
        <Button
          size="sm"
          disabled={!valid}
          onClick={() => {
            onChange({ ...state, range: "custom", from, to });
          }}
        >
          Apply
        </Button>
      </div>
    </fieldset>
  );
}

/** One panel for every filter (§2.2); changes apply immediately and live in the URL. */
export function FiltersSheet({
  open,
  onOpenChange,
  state,
  options,
  onChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  state: FilterState;
  options: FilterOptions;
  onChange: (next: FilterState) => void;
}) {
  return (
    <Sheet
      open={open}
      onOpenChange={onOpenChange}
      title="Filters"
      description="Only values recorded in the selected period are listed."
    >
      <div className="grid gap-6">
        <CustomPeriod state={state} onChange={onChange} />
        {DIMENSIONS.filter((d) => options[d.key].length > 0).map((dimension) => (
          <fieldset key={dimension.key} className="grid gap-1.5">
            <legend className="mb-1 text-xs font-medium tracking-wide text-subtle uppercase">
              {dimension.label}
            </legend>
            {options[dimension.key].map((option) => (
              <label key={option.value} className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  className="size-4 accent-[var(--accent)]"
                  checked={state.dimensions[dimension.key].includes(option.value)}
                  onChange={() => {
                    onChange(toggleValue(state, dimension.key, option.value));
                  }}
                />
                <span className="truncate">{option.label}</span>
              </label>
            ))}
          </fieldset>
        ))}
        <div>
          <Button
            variant="ghost"
            onClick={() => {
              onChange(clearDimensions(state));
            }}
          >
            Clear all filters
          </Button>
        </div>
      </div>
    </Sheet>
  );
}
