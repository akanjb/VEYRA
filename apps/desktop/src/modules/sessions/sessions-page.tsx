import type { SessionSort } from "@veyra/contracts";
import { ChevronLeft, ChevronRight } from "lucide-react";

import { useCommand } from "@/shared/api";
import { DataTable } from "@/shared/charts";
import { useFilters, useRawSearch, useReplaceSearch } from "@/shared/filters";
import { fmt } from "@/shared/format";
import { Button, Card, EmptyState, ErrorState, Segmented, Skeleton } from "@/shared/ui";

import { SESSION_COLUMNS } from "./session-columns";
import { useOpenSession } from "./session-links";
import {
  PAGE_SIZE,
  SESSIONS_SEARCH_KEYS,
  SORTS,
  validateSessionsSearch,
  type SessionsSearch,
} from "./sessions-search";

/** Sort and page from the URL; updates drop defaults and keep the global filters. */
function usePaging() {
  const search = validateSessionsSearch(useRawSearch());
  const replace = useReplaceSearch(SESSIONS_SEARCH_KEYS);
  const update = (next: SessionsSearch) => {
    replace(validateSessionsSearch({ ...next }));
  };
  return { sort: search.sort ?? "lastActive", page: search.page ?? 0, update };
}

function Pager({
  page,
  total,
  onPage,
}: {
  page: number;
  total: number;
  onPage: (page: number) => void;
}) {
  const first = page * PAGE_SIZE + 1;
  const last = Math.min(total, (page + 1) * PAGE_SIZE);
  return (
    <div className="flex items-center justify-end gap-2 text-xs text-muted">
      <span data-numeric>
        {fmt.integer(first)}–{fmt.integer(last)} of {fmt.integer(total)}
      </span>
      <Button
        size="sm"
        aria-label="Previous page"
        disabled={page === 0}
        onClick={() => {
          onPage(page - 1);
        }}
      >
        <ChevronLeft size={14} aria-hidden />
      </Button>
      <Button
        size="sm"
        aria-label="Next page"
        disabled={last >= total}
        onClick={() => {
          onPage(page + 1);
        }}
      >
        <ChevronRight size={14} aria-hidden />
      </Button>
    </div>
  );
}

/** "Which sessions drove usage?" The explorer, sorted and paged by the core. */
export function SessionsPage() {
  const { scope } = useFilters();
  const { sort, page, update } = usePaging();
  const openSession = useOpenSession();
  const list = useCommand("list_sessions", { scope, sort, page, pageSize: PAGE_SIZE });
  const setSort = (next: SessionSort) => {
    update({ sort: next });
  };
  return (
    <Card
      title="Sessions"
      definition="Sessions with at least one request in the period. Numbers cover requests inside the period; open a session for its whole timeline."
      actions={
        <Segmented label="Sort sessions by" value={sort} options={SORTS} onChange={setSort} />
      }
    >
      {list.isPending ? (
        <Skeleton className="h-96" />
      ) : list.isError ? (
        <ErrorState error={list.error} onRetry={() => void list.refetch()} />
      ) : list.data.total === 0 ? (
        <EmptyState title="No sessions in this period">
          Try a longer period or remove filters.
        </EmptyState>
      ) : (
        <div className="grid gap-3">
          <div className="overflow-x-auto">
            <DataTable
              caption="Sessions in the period"
              columns={SESSION_COLUMNS}
              rows={list.data.rows}
              rowKey={(r) => String(r.id)}
              onRowClick={(r) => {
                openSession(r.id);
              }}
            />
          </div>
          <Pager
            page={page}
            total={list.data.total}
            onPage={(p) => {
              update({ sort, page: p });
            }}
          />
        </div>
      )}
    </Card>
  );
}
