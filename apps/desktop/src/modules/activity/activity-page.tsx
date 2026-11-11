import { useCommand } from "@/shared/api";
import { useFilters, withDay } from "@/shared/filters";
import { CalendarCard, HoursCard } from "@/shared/time-cards";
import { CardGrid, ErrorState, NoUsageCard, PageSkeleton } from "@/shared/ui";

import {
  ActivityKpiStrip,
  DailyTimeCard,
  LengthsCard,
  LongestCard,
  PeaksCard,
} from "./activity-cards";
import { hasActivity } from "./activity-model";

/** "When do I work with AI?" (docs/planning/03-ux.md §4, Activity). */
export function ActivityPage() {
  const filters = useFilters();
  const activity = useCommand("get_activity", { scope: filters.scope });
  if (activity.isPending) return <PageSkeleton label="Loading activity" />;
  if (activity.isError)
    return <ErrorState error={activity.error} onRetry={() => void activity.refetch()} />;

  const data = activity.data;
  const { state, setState } = filters;
  return (
    <div className="grid gap-4">
      <ActivityKpiStrip activity={data} />
      <p className="text-xs text-muted">
        Active time caps each gap between requests at {String(data.idleThresholdMs / 60_000)}{" "}
        minutes; wall time counts every gap in full.
      </p>
      {hasActivity(data) ? (
        <>
          <DailyTimeCard activity={data} />
          <CardGrid>
            <LengthsCard activity={data} />
            <PeaksCard activity={data} />
          </CardGrid>
          <LongestCard activity={data} />
        </>
      ) : (
        <NoUsageCard title="Activity" />
      )}
      <CardGrid>
        <CalendarCard
          calendar={data.calendar}
          accuracy={null}
          onSelectDate={(date) => {
            setState(withDay(state, date));
          }}
        />
        <HoursCard hours={data.hours} accuracy={null} />
      </CardGrid>
    </div>
  );
}
