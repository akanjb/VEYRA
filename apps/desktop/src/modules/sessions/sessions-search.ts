/** URL state of the session explorer: sort order and page (filters are global). */
import type { SessionSort } from "@veyra/contracts";

export const SORTS: readonly { value: SessionSort; label: string }[] = [
  { value: "lastActive", label: "Recent" },
  { value: "tokens", label: "Tokens" },
  { value: "cost", label: "Cost" },
  { value: "events", label: "Requests" },
];

export const PAGE_SIZE = 50;

/** The search keys this page owns (the global filters own the rest). */
export const SESSIONS_SEARCH_KEYS = ["sort", "page"] as const;

export interface SessionsSearch {
  sort?: SessionSort;
  page?: number;
}

/** Keeps a known sort (not the default) and a positive page (not the first). */
export function validateSessionsSearch(raw: Record<string, unknown>): SessionsSearch {
  const search: SessionsSearch = {};
  const sort = SORTS.find((s) => s.value === raw["sort"])?.value;
  if (sort !== undefined && sort !== "lastActive") search.sort = sort;
  const page = Number(raw["page"]);
  if (Number.isInteger(page) && page > 0 && page < 100_000) search.page = page;
  return search;
}
