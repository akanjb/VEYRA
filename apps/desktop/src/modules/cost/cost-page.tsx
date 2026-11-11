import type { BreakdownDimension, Cost, Scope } from "@veyra/contracts";
import { useState } from "react";

import { useCommand } from "@/shared/api";
import { toggleValue, useFilters, withDay } from "@/shared/filters";
import { ExpensiveSessionsCard } from "@/shared/summary";
import { CardGrid, ErrorState, NoUsageCard, PageSkeleton } from "@/shared/ui";

import {
  CompositionCard,
  CostBreakdownCard,
  DimensionTabs,
  RatesCard,
  WaterfallCard,
} from "./cost-breakdowns";
import {
  BudgetsCard,
  CostKpiStrip,
  CumulativeCard,
  DailyCostCard,
  MonthOutlookCard,
  ValueFraming,
} from "./cost-cards";
import { BREAKDOWN, hasCost } from "./cost-model";
import { AssumptionsButton } from "./assumptions-sheet";

/** The value framing, and the way into how the cost was estimated. */
function FramingRow({ data, scope }: { data: Cost; scope: Scope }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-2">
      <ValueFraming data={data} />
      <AssumptionsButton scope={scope} />
    </div>
  );
}

/** "What does it cost: estimated, actual, subscription?" (docs/planning/03-ux.md §4, Cost). */
export function CostPage() {
  const filters = useFilters();
  const query = useCommand("get_cost", { scope: filters.scope });
  const [dimension, setDimension] = useState<BreakdownDimension>("model");
  const [change, setChange] = useState<BreakdownDimension>("model");
  if (query.isPending) return <PageSkeleton label="Loading cost" />;
  if (query.isError) return <ErrorState error={query.error} onRetry={() => void query.refetch()} />;

  const data = query.data;
  const { state, setState } = filters;
  const filter = BREAKDOWN[dimension]?.filter;
  return (
    <div className="grid gap-4">
      <CostKpiStrip data={data} />
      <FramingRow data={data} scope={filters.scope} />
      <CardGrid>
        <MonthOutlookCard data={data} />
        <BudgetsCard data={data} />
      </CardGrid>
      {hasCost(data) ? (
        <>
          <CardGrid>
            <DailyCostCard
              data={data}
              onSelectDate={(date) => {
                setState(withDay(state, date));
              }}
            />
            <CumulativeCard data={data} />
          </CardGrid>
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs text-muted">Break down by</span>
            <DimensionTabs value={dimension} onChange={setDimension} />
          </div>
          <CardGrid>
            <CostBreakdownCard
              data={data}
              dimension={dimension}
              onSelect={(key) => {
                if (filter !== undefined) setState(toggleValue(state, filter, key));
              }}
            />
            <WaterfallCard data={data} dimension={change} onDimension={setChange} />
          </CardGrid>
          <CardGrid>
            <CompositionCard data={data} />
            <RatesCard data={data} />
          </CardGrid>
          <ExpensiveSessionsCard
            sessions={data.sessions}
            definition="Sessions with the highest API-equivalent cost in the period."
          />
        </>
      ) : (
        <NoUsageCard title="Cost" />
      )}
    </div>
  );
}
