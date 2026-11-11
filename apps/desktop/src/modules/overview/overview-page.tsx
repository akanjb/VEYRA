import { useCommand } from "@/shared/api";
import { toggleValue, useFilters, withDay } from "@/shared/filters";
import { KpiStrip, ModelMixCard, TrendCard, hasUsage } from "@/shared/summary";
import { CalendarCard, HoursCard } from "@/shared/time-cards";
import { CardGrid, ErrorState, NoUsageCard, PageSkeleton } from "@/shared/ui";

import { ProjectsCard } from "./overview-cards";

/** "How am I using AI overall?" (docs/planning/03-ux.md §4, Overview). */
export function OverviewPage() {
  const filters = useFilters();
  const overview = useCommand("get_overview", { scope: filters.scope });
  if (overview.isPending) return <PageSkeleton label="Loading overview" />;
  if (overview.isError)
    return <ErrorState error={overview.error} onRetry={() => void overview.refetch()} />;

  const data = overview.data;
  const { state, setState } = filters;
  const selectDay = (date: string) => {
    setState(withDay(state, date));
  };
  return (
    <div className="grid gap-4">
      <KpiStrip summary={data} measure={state.measure} />
      {hasUsage(data) ? (
        <>
          <TrendCard summary={data} measure={state.measure} onSelectDate={selectDay} />
          <CardGrid>
            <ModelMixCard
              summary={data}
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
          </CardGrid>
        </>
      ) : (
        <NoUsageCard />
      )}
      <CardGrid>
        <CalendarCard
          calendar={data.calendar}
          accuracy={data.composition.weakest}
          onSelectDate={selectDay}
        />
        <HoursCard hours={data.hours} accuracy={data.composition.weakest} />
      </CardGrid>
    </div>
  );
}
