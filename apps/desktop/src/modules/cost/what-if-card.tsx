/** "What would this period cost on another model?": the period's tokens repriced. */
import type { Scope, WhatIf, WhatIfComparison, WhatIfRow } from "@veyra/contracts";
import { useState } from "react";

import { useCommand } from "@/shared/api";
import { DataTable, type Column } from "@/shared/charts";
import { MEASURE, MISSING, fmt } from "@/shared/format";
import { CONTROL, Card, EmptyState, ErrorState, Skeleton } from "@/shared/ui";

import { changeText, choiceGroups, leftOutText, summaryText } from "./what-if-model";

const cost = MEASURE.cost.exact;

const COLUMNS: readonly Column<WhatIfRow>[] = [
  { header: "Model used", cell: (r) => r.label },
  { header: "Tokens", cell: (r) => fmt.compact(r.tokens), numeric: true },
  { header: "As used", cell: (r) => cost(r.currentNanoUsd), numeric: true },
  { header: "On the other model", cell: (r) => cost(r.repricedNanoUsd), numeric: true },
  { header: "Change", cell: (r) => changeText(r.change) ?? MISSING, numeric: true },
];

function ModelSelect({
  data,
  value,
  onChange,
}: {
  data: WhatIf;
  value: string | null;
  onChange: (model: string | null) => void;
}) {
  return (
    <select
      aria-label="Model to price the period on"
      className={`${CONTROL} w-full sm:w-72`}
      value={value ?? ""}
      onChange={(e) => {
        onChange(e.target.value === "" ? null : e.target.value);
      }}
    >
      <option value="">Choose a model…</option>
      {choiceGroups(data.choices).map((group) => (
        <optgroup key={group.provider} label={group.provider}>
          {group.choices.map((c) => (
            <option key={c.model.key} value={c.model.key}>
              {c.model.label}
            </option>
          ))}
        </optgroup>
      ))}
    </select>
  );
}

function Comparison({ comparison }: { comparison: WhatIfComparison }) {
  const notes = [leftOutText(comparison), ...comparison.assumptions].filter((n) => n !== null);
  return (
    <div className="grid gap-3">
      <p className="text-sm font-medium" data-numeric>
        {summaryText(comparison)}
      </p>
      <div className="max-h-80 overflow-auto">
        <DataTable
          columns={COLUMNS}
          rows={comparison.rows}
          rowKey={(r, i) => r.key ?? `unknown-${String(i)}`}
          caption={`Cost of each model's tokens as used and on ${comparison.target.model.label}`}
        />
      </div>
      {notes.length === 0 ? null : (
        <ul className="grid list-disc gap-1 pl-4 text-xs text-muted">
          {notes.map((note) => (
            <li key={note}>{note}</li>
          ))}
        </ul>
      )}
    </div>
  );
}

export function WhatIfCard({ scope }: { scope: Scope }) {
  const [model, setModel] = useState<string | null>(null);
  const query = useCommand("get_what_if", { scope, model });
  const title = "What if another model did the work";
  if (query.isPending) {
    return (
      <Card title={title}>
        <Skeleton className="h-24" />
      </Card>
    );
  }
  if (query.isError) {
    return (
      <Card title={title}>
        <ErrorState error={query.error} onRetry={() => void query.refetch()} />
      </Card>
    );
  }
  const data = query.data;
  return (
    <Card title={title} definition={data.definition} accuracy="estimated">
      <div className="grid gap-3">
        <ModelSelect data={data} value={model} onChange={setModel} />
        {data.comparison === null ? (
          <EmptyState title="Choose a model">
            See what this period's tokens would cost at that model's current published prices.
          </EmptyState>
        ) : data.comparison.rows.length === 0 ? (
          <EmptyState title="Nothing to compare">
            No model in this period has a published price for every request.
          </EmptyState>
        ) : (
          <Comparison comparison={data.comparison} />
        )}
      </div>
    </Card>
  );
}
