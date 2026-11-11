import type { Overview } from "@veyra/contracts";

import { RankTable, Treemap, useChartTheme } from "@/shared/charts";
import type { Measure } from "@/shared/filters";
import { MEASURE } from "@/shared/format";
import { ChartCard } from "@/shared/ui";

import { projectItems } from "./overview-model";

interface CardProps {
  overview: Overview;
  measure: Measure;
}

export function ProjectsCard({
  overview,
  measure,
  onSelect,
}: CardProps & { onSelect: (project: string) => void }) {
  const theme = useChartTheme();
  const items = projectItems(overview, measure, theme);
  const meta = MEASURE[measure];
  return (
    <ChartCard
      title="Projects"
      definition="Usage per project (working directory), top 30 plus everything else. Click a project to filter by it."
      accuracy={meta.estimated ? "estimated" : overview.composition.weakest}
      chart={
        <Treemap items={items} format={meta.format} label="Usage by project" onSelect={onSelect} />
      }
      table={
        <RankTable items={items} format={meta.exact} caption="Usage by project" header="Project" />
      }
    />
  );
}
