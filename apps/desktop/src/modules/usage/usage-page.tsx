import type { BreakdownDimension } from "@veyra/contracts";
import { useState } from "react";

import { useCommand } from "@/shared/api";
import { toggleValue, useFilters, withDay } from "@/shared/filters";
import { CardGrid, ErrorState, NoUsageCard, PageSkeleton } from "@/shared/ui";

import {
  BreakdownCard,
  CompositionCard,
  ConcentrationCard,
  DimensionTabs,
  RequestDistributionCard,
  RequestSizeCard,
  SessionSizeCard,
  UsageKpiStrip,
} from "./usage-cards";
import { BREAKDOWN, hasUsage } from "./usage-model";

/** "What is consumed, and where?" (docs/planning/03-ux.md §4, Usage). */
export function UsagePage() {
  const filters = useFilters();
  const usage = useCommand("get_usage", { scope: filters.scope });
  const [dimension, setDimension] = useState<BreakdownDimension>("model");
  if (usage.isPending) return <PageSkeleton label="Loading usage" />;
  if (usage.isError) return <ErrorState error={usage.error} onRetry={() => void usage.refetch()} />;

  const data = usage.data;
  const { state, setState } = filters;
  const filter = BREAKDOWN[dimension].filter;
  const daily = data.composition.map(
    (d) => d.inputUncached + d.cacheRead + d.cacheWrite + d.output,
  );
  return (
    <div className="grid gap-4">
      <UsageKpiStrip usage={data} trend={daily} />
      {hasUsage(data) ? (
        <>
          <CompositionCard
            usage={data}
            onSelectDate={(date) => {
              setState(withDay(state, date));
            }}
          />
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs text-muted">Break down by</span>
            <DimensionTabs value={dimension} onChange={setDimension} />
          </div>
          <CardGrid>
            <BreakdownCard
              usage={data}
              dimension={dimension}
              onSelect={
                filter === null
                  ? undefined
                  : (key) => {
                      setState(toggleValue(state, filter, key));
                    }
              }
            />
            <ConcentrationCard usage={data} dimension={dimension} />
          </CardGrid>
          <CardGrid>
            <RequestSizeCard usage={data} />
            <SessionSizeCard usage={data} />
          </CardGrid>
          <RequestDistributionCard usage={data} />
        </>
      ) : (
        <NoUsageCard />
      )}
    </div>
  );
}
