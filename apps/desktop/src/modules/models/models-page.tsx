import { useState } from "react";

import { useCommand } from "@/shared/api";
import { useFilters } from "@/shared/filters";
import { CardGrid, ErrorState, NoUsageCard, PageSkeleton } from "@/shared/ui";

import { CompareCard, LeaderboardCard, MatrixCard, ShareCard, TimelineCard } from "./models-cards";
import { COMPARE_MIN, hasModels, rowId, toggleCompared } from "./models-model";

/**
 * "Which models do I use and how do they compare?"
 * (docs/planning/03-ux.md §4, Models & Providers).
 */
export function ModelsPage() {
  const filters = useFilters();
  const models = useCommand("get_models", { scope: filters.scope });
  const [compared, setCompared] = useState<string[]>([]);
  if (models.isPending) return <PageSkeleton label="Loading models" />;
  if (models.isError)
    return <ErrorState error={models.error} onRetry={() => void models.refetch()} />;

  const data = models.data;
  if (!hasModels(data)) return <NoUsageCard title="Models & Providers" />;
  const comparedRows = compared.flatMap((id) => data.leaderboard.filter((r) => rowId(r) === id));
  return (
    <div className="grid gap-4">
      <ShareCard models={data} />
      <LeaderboardCard
        models={data}
        compared={compared}
        onToggle={(id) => {
          setCompared((current) => toggleCompared(current, id));
        }}
      />
      {comparedRows.length >= COMPARE_MIN ? <CompareCard rows={comparedRows} /> : null}
      <CardGrid>
        <MatrixCard
          title="Providers by application"
          definition="Tokens per provider (rows) in each application (columns)."
          corner="Provider"
          matrix={data.providerByApp}
          accuracy={data.accuracy}
        />
        <MatrixCard
          title="Models by application"
          definition="Tokens per model (rows, the most used) in each application (columns)."
          corner="Model"
          matrix={data.modelByApp}
          accuracy={data.accuracy}
        />
      </CardGrid>
      <TimelineCard models={data} />
    </div>
  );
}
