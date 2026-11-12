import { useCommand } from "@/shared/api";
import { useFilters, withDay } from "@/shared/filters";
import { CardGrid, ErrorState, NoUsageCard, PageSkeleton } from "@/shared/ui";

import { FailingCard, KindsCard, ServersCard, ToolsKpiStrip, TopToolsCard } from "./tools-cards";
import { hasToolCalls } from "./tools-model";

/** "What do the agents do with their tools, and which ones fail?" (ADR-034). */
export function ToolsPage() {
  const filters = useFilters();
  const tools = useCommand("get_tools", { scope: filters.scope });
  if (tools.isPending) return <PageSkeleton label="Loading tools" />;
  if (tools.isError) return <ErrorState error={tools.error} onRetry={() => void tools.refetch()} />;

  const data = tools.data;
  const { state, setState } = filters;
  const daily = data.daily.map((d) => d.calls.reduce((sum, calls) => sum + calls, 0));
  return (
    <div className="grid gap-4">
      <ToolsKpiStrip tools={data} trend={daily} />
      {hasToolCalls(data) ? (
        <>
          <KindsCard
            tools={data}
            onSelectDate={(date) => {
              setState(withDay(state, date));
            }}
          />
          <TopToolsCard tools={data} />
          <CardGrid>
            <ServersCard tools={data} />
            <FailingCard tools={data} />
          </CardGrid>
        </>
      ) : (
        <NoUsageCard title="Tool calls" />
      )}
    </div>
  );
}
