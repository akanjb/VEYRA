import { useCommand } from "@/shared/api";
import { toggleValue, useFilters, withDay } from "@/shared/filters";
import { Card, EmptyState, ErrorState, Skeleton } from "@/shared/ui";

import { KpiStrip } from "./kpi-strip";
import { CalendarCard, HoursCard, ModelMixCard, ProjectsCard, TrendCard } from "./overview-cards";
import { hasUsage } from "./overview-model";

function Loading() {
  return (
    <div className="grid gap-4" aria-busy="true" aria-label="Loading overview">
      <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
        {Array.from({ length: 4 }, (_, i) => (
          <Skeleton key={i} className="h-28" />
        ))}
      </div>
      <Skeleton className="h-80" />
    </div>
  );
}

/** "How am I using AI overall?" (docs/planning/03-ux.md §4, Overview). */
export function OverviewPage() {
  const filters = useFilters();
  const overview = useCommand("get_overview", { scope: filters.scope });
  if (overview.isPending) return <Loading />;
  if (overview.isError)
    return <ErrorState error={overview.error} onRetry={() => void overview.refetch()} />;

  const data = overview.data;
  const { state, setState } = filters;
  const selectDay = (date: string) => {
    setState(withDay(state, date));
  };
  return (
    <div className="grid gap-4">
      <KpiStrip overview={data} measure={state.measure} />
      {hasUsage(data) ? (
        <>
          <TrendCard overview={data} measure={state.measure} onSelectDate={selectDay} />
          <div className="grid gap-4 xl:grid-cols-2">
            <ModelMixCard
              overview={data}
              measure={state.measure}
              onSelect={(m) => {
                setState(toggleValue(state, "models", m));
              }}
            />
            <ProjectsCard
              overview={data}
              measure={state.measure}
              onSelect={(p) => {
                setState(toggleValue(state, "projects", p));
              }}
            />
          </div>
        </>
      ) : (
        <Card title="Usage">
          <EmptyState title="No usage recorded in this period">
            Try a longer period, remove filters, or check that a source is enabled under Sources
            &amp; Health.
          </EmptyState>
        </Card>
      )}
      <div className="grid gap-4 xl:grid-cols-2">
        <CalendarCard overview={data} onSelectDate={selectDay} />
        <HoursCard overview={data} />
      </div>
    </div>
  );
}
