import type { Highlight } from "@veyra/contracts";

import { Card } from "@/shared/ui";

import { highlightText } from "./highlights-model";

/** Up to five facts about the period, each restating a figure the page shows. */
export function HighlightsCard({ highlights }: { highlights: readonly Highlight[] }) {
  if (highlights.length === 0) return null;
  return (
    <Card
      title="Highlights"
      definition="Facts about the period, chosen by fixed rules from the figures on this page: the busiest day, the top model and project, what caching saved and how cost changed."
    >
      <ul className="flex flex-wrap gap-2" aria-label="Highlights">
        {highlights.map((h) => {
          const text = highlightText(h);
          return (
            <li
              key={h.kind}
              className="flex items-baseline gap-2 rounded-control border border-border bg-sunken px-3 py-1.5 text-sm"
            >
              <span className="text-muted">{text.label}</span>
              <span className="font-medium" data-numeric>
                {text.value}
              </span>
            </li>
          );
        })}
      </ul>
    </Card>
  );
}
