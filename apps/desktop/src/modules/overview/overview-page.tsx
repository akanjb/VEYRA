import { useCommand } from "@/shared/api";
import { toggleValue, useFilters, withDay } from "@/shared/filters";
import { CalendarCard, HoursCard } from "@/shared/time-cards";
import { CardGrid, ErrorState, NoUsageCard, PageSkeleton } from "@/shared/ui";

import { KpiStrip } from "./kpi-strip";
import { ModelMixCard, ProjectsCard, TrendCard } from "./overview-cards";
import { hasUsage } from "./overview-model";

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
      <KpiStrip overview={data} measure={state.measure} />
      {hasUsage(data) ? (
        <>
          <TrendCard overview={data} measure={state.measure} onSelectDate={selectDay} />
          <CardGrid>
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
