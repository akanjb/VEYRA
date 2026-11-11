import type { SessionRow } from "@veyra/contracts";

import { DataTable } from "../charts";
import { MEASURE, MISSING, fmt } from "../format";
import { useOpenSession } from "../links";
import { Card } from "../ui";

/** The sessions with the highest API-equivalent cost; a row opens its detail. */
export function ExpensiveSessionsCard({
  sessions,
  definition,
}: {
  sessions: readonly SessionRow[];
  definition: string;
}) {
  const open = useOpenSession();
  return (
    <Card title="Most expensive sessions" definition={definition} accuracy="estimated">
      <DataTable<SessionRow>
        caption="Most expensive sessions"
        rows={sessions}
        rowKey={(r) => String(r.id)}
        onRowClick={(r) => {
          open(r.id);
        }}
        columns={[
          { header: "Started", cell: (r) => fmt.dateTime(r.startedAt) },
          { header: "Models", cell: (r) => r.models.map((m) => m.label).join(", ") || MISSING },
          {
            header: "Tokens",
            numeric: true,
            cell: (r) => MEASURE.tokens.format(r.measures.tokens),
          },
          {
            header: "Cost",
            numeric: true,
            cell: (r) => MEASURE.cost.exact(r.measures.costNanoUsd),
          },
          { header: "Active time", numeric: true, cell: (r) => fmt.duration(r.activeMs) },
        ]}
      />
    </Card>
  );
}
