import { useState, type ReactNode } from "react";
import { Info, Table2, LineChart, TriangleAlert } from "lucide-react";

import { ApiCallError } from "../api";
import { AccuracyBadge, type AccuracyLabel } from "./accuracy";
import { Button, Tip, cn } from "./primitives";

interface CardProps {
  title: string;
  /** The metric definition, shown from the ⓘ icon. */
  definition?: string;
  accuracy?: AccuracyLabel | null;
  actions?: ReactNode;
  className?: string;
  children: ReactNode;
}

/** A dashboard card: title, definition, accuracy and actions over its content. */
export function Card({
  title,
  definition,
  accuracy = null,
  actions,
  className,
  children,
}: CardProps) {
  return (
    <section
      aria-label={title}
      className={cn(
        "flex min-w-0 flex-col rounded-card border border-border bg-surface",
        className,
      )}
    >
      <header className="flex min-h-11 items-center gap-2 border-b border-border px-4 py-2">
        <h2 className="truncate text-sm font-medium">{title}</h2>
        {definition === undefined ? null : (
          <Tip content={definition}>
            <button
              type="button"
              aria-label={`About ${title}`}
              className="text-subtle hover:text-text"
            >
              <Info size={14} aria-hidden />
            </button>
          </Tip>
        )}
        <AccuracyBadge kind={accuracy} />
        <div className="ml-auto flex items-center gap-1">{actions}</div>
      </header>
      <div className="min-h-0 flex-1 p-4">{children}</div>
    </section>
  );
}

/**
 * A card body that shows a chart or its table twin (every chart has one: §5.2).
 * `chart` and `table` render the same data.
 */
export function ChartCard(
  props: Omit<CardProps, "children"> & { chart: ReactNode; table: ReactNode },
) {
  const [asTable, setAsTable] = useState(false);
  return (
    <Card
      {...props}
      actions={
        <>
          {props.actions}
          <Tip content={asTable ? "Show chart" : "Show table"}>
            <Button
              variant="ghost"
              size="sm"
              aria-label={
                asTable ? `Show ${props.title} as a chart` : `Show ${props.title} as a table`
              }
              aria-pressed={asTable}
              onClick={() => {
                setAsTable((v) => !v);
              }}
            >
              {asTable ? <LineChart size={14} aria-hidden /> : <Table2 size={14} aria-hidden />}
            </Button>
          </Tip>
        </>
      }
    >
      {asTable ? <div className="max-h-80 overflow-auto">{props.table}</div> : props.chart}
    </Card>
  );
}

export function EmptyState({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="grid place-items-center gap-1 py-10 text-center">
      <p className="text-sm font-medium">{title}</p>
      {children === undefined ? null : (
        <div className="max-w-md text-sm text-muted">{children}</div>
      )}
    </div>
  );
}

/** A card-level error with retry; shows the correlation id for support, never internals. */
export function ErrorState({ error, onRetry }: { error: Error; onRetry?: () => void }) {
  const detail =
    error instanceof ApiCallError
      ? error.error.kind === "internal"
        ? `Something went wrong in the Veyra core (reference ${error.error.correlationId}).`
        : error.error.message
      : "The Veyra core could not be reached.";
  return (
    <div role="alert" className="grid place-items-center gap-2 py-8 text-center">
      <TriangleAlert size={18} className="text-warning" aria-hidden />
      <p className="max-w-md text-sm text-muted">{detail}</p>
      {onRetry === undefined ? null : (
        <Button size="sm" onClick={onRetry}>
          Retry
        </Button>
      )}
    </div>
  );
}
