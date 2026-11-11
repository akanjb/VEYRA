/** Building blocks every analytics page shares (docs/planning/03-ux.md §3). */
import type { ReactNode } from "react";

import { Card, EmptyState } from "./card";
import { Skeleton } from "./primitives";

/** The KPI strip: the first tile is the hero, wider on large screens. */
export function KpiGrid({ children }: { children: ReactNode }) {
  return (
    <div className="grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-[minmax(0,2fr)_repeat(5,minmax(0,1fr))]">
      {children}
    </div>
  );
}

/** Placeholders shaped like a page: a KPI strip and a primary chart. */
export function PageSkeleton({ label }: { label: string }) {
  return (
    <div className="grid gap-4" aria-busy="true" aria-label={label}>
      <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
        {Array.from({ length: 4 }, (_, i) => (
          <Skeleton key={i} className="h-28" />
        ))}
      </div>
      <Skeleton className="h-80" />
    </div>
  );
}

/** Shown in place of a page's charts when the scope holds no usage. */
export function NoUsageCard({ title = "Usage" }: { title?: string }) {
  return (
    <Card title={title}>
      <EmptyState title="No usage recorded in this period">
        Try a longer period, remove filters, or check that a source is enabled under Sources &amp;
        Health.
      </EmptyState>
    </Card>
  );
}

/** Two cards side by side on wide screens, stacked below 1100 px. */
export function CardGrid({ children }: { children: ReactNode }) {
  return <div className="grid gap-4 xl:grid-cols-2">{children}</div>;
}
