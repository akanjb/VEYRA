/**
 * Reading and replacing URL search params. Each consumer owns a set of keys (the global
 * filters, a page's sort and paging) and replaces only those, keeping everyone else's.
 */
import { useCallback } from "react";
import { useNavigate, useSearch } from "@tanstack/react-router";

/** The current search params, unvalidated: each consumer validates its own keys. */
export function useRawSearch(): Record<string, unknown> {
  const search: Record<string, unknown> = useSearch({ strict: false });
  return search;
}

/** `record` without `keys`. */
export function omit(
  record: Record<string, unknown>,
  keys: readonly string[],
): Record<string, unknown> {
  return Object.fromEntries(Object.entries(record).filter(([k]) => !keys.includes(k)));
}

/**
 * A setter that replaces the `owned` keys with `next` on the current page (a new history
 * entry, so Back restores the previous state). `owned` must be a stable constant.
 */
export function useReplaceSearch(owned: readonly string[]): (next: object) => void {
  const navigate = useNavigate();
  return useCallback(
    (next: object) => {
      void navigate({
        to: ".",
        search: (previous: Record<string, unknown>) => ({ ...omit(previous, owned), ...next }),
      } as never);
    },
    [navigate, owned],
  );
}
