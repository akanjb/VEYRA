import { useMemo } from "react";
import type { Scope } from "@veyra/contracts";

import { systemTimeZone, todayIn } from "./dates";
import {
  FILTER_KEYS,
  filterState,
  toScope,
  toSearch,
  validateSearch,
  type FilterState,
  type SearchParams,
} from "./filters";
import { useRawSearch, useReplaceSearch } from "./url";

export interface Filters {
  state: FilterState;
  scope: Scope;
  today: string;
  timeZone: string;
  /** Replaces the filter state in the URL (a new history entry, so Back restores it). */
  setState: (next: FilterState) => void;
}

/** The global filters of the current URL, and how to change them. */
export function useFilters(): Filters {
  const key = JSON.stringify(validateSearch(useRawSearch()));
  const replace = useReplaceSearch(FILTER_KEYS);
  const timeZone = systemTimeZone();
  const today = todayIn(timeZone);
  return useMemo(() => {
    const state = filterState(JSON.parse(key) as SearchParams);
    return {
      state,
      scope: toScope(state, today, timeZone),
      today,
      timeZone,
      setState: (next: FilterState) => {
        replace(toSearch(next));
      },
    };
  }, [key, today, timeZone, replace]);
}
