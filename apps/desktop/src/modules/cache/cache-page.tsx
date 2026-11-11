import { useCommand } from "@/shared/api";
import { toggleValue, useFilters } from "@/shared/filters";
import { CardGrid, ErrorState, NoUsageCard, PageSkeleton } from "@/shared/ui";

import {
  CacheKpiStrip,
  CoverageNote,
  FlowCard,
  HitRatioCard,
  LossesCard,
  TtlCard,
} from "./cache-cards";
import { hasCacheUse } from "./cache-model";

/** "Is caching working, and what does it save?" (docs/planning/03-ux.md §4, Cache). */
export function CachePage() {
  const filters = useFilters();
  const cache = useCommand("get_cache", { scope: filters.scope });
  if (cache.isPending) return <PageSkeleton label="Loading cache" />;
  if (cache.isError) return <ErrorState error={cache.error} onRetry={() => void cache.refetch()} />;

  const data = cache.data;
  const { state, setState } = filters;
  return (
    <div className="grid gap-4">
      <CacheKpiStrip cache={data} trend={data.daily.map((d) => d.reads)} />
      <CoverageNote cache={data} />
      {hasCacheUse(data) ? (
        <>
          <FlowCard cache={data} />
          <CardGrid>
            <HitRatioCard
              cache={data}
              by="model"
              onSelect={(key) => {
                setState(toggleValue(state, "models", key));
              }}
            />
            <HitRatioCard
              cache={data}
              by="project"
              onSelect={(key) => {
                setState(toggleValue(state, "projects", key));
              }}
            />
          </CardGrid>
          <TtlCard cache={data} />
          <LossesCard cache={data} />
        </>
      ) : (
        <NoUsageCard title="Cache" />
      )}
    </div>
  );
}
