/** Maps the Overview's project distribution to chart items for the selected measure. */
import type { Overview } from "@veyra/contracts";

import type { ChartTheme, RankItem } from "@/shared/charts";
import type { Measure } from "@/shared/filters";
import { MEASURE } from "@/shared/format";
import { rankItems } from "@/shared/rankings";

/** Project distribution; each project keeps a colour by rank. */
export function projectItems(overview: Overview, measure: Measure, theme: ChartTheme): RankItem[] {
  return rankItems(
    overview.projects,
    { pick: MEASURE[measure].pick, coloring: "category", noun: "projects" },
    theme,
  );
}
