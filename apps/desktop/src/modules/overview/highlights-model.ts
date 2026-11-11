/** Words for the Overview's highlights. Pure: each kind is phrased from the core's figures. */
import type { Highlight } from "@veyra/contracts";

import { MEASURE, fmt } from "@/shared/format";

/** One highlight as a short label and its value. */
export interface HighlightText {
  label: string;
  value: string;
}

export function highlightText(highlight: Highlight): HighlightText {
  switch (highlight.kind) {
    case "busiestDay":
      return {
        label: `Busiest day: ${fmt.day(highlight.date)}`,
        value: `${MEASURE.tokens.format(highlight.tokens)} tokens`,
      };
    case "topModel":
      return {
        label: `Top model: ${highlight.label}`,
        value: `${fmt.percent(highlight.share)} of tokens`,
      };
    case "topProject":
      return {
        label: `Top project: ${highlight.label}`,
        value: `${fmt.percent(highlight.share)} of tokens`,
      };
    case "cacheSaved":
      return { label: "Cache saved", value: MEASURE.cost.format(highlight.netNanoUsd) };
    case "costChange":
      return {
        label: highlight.change >= 0 ? "Cost up" : "Cost down",
        value: `${fmt.change(highlight.change)} vs previous period`,
      };
  }
}
