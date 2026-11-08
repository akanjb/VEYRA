/** Column definitions shared by the explorer and the subagent list. */
import type { SessionRow } from "@veyra/contracts";

import type { Column } from "@/shared/charts";
import { MEASURE, fmt } from "@/shared/format";
import { Tip } from "@/shared/ui";

import { modelNames } from "./session-model";

function Cost({ row }: { row: SessionRow }) {
  const value = MEASURE.cost.format(row.measures.costNanoUsd);
  if (row.unpricedRequests === 0 && !row.costAssumptions) return <>{value}</>;
  const why =
    row.unpricedRequests > 0
      ? `${fmt.integer(row.unpricedRequests)} request(s) could not be priced, so this is a lower bound.`
      : "Some requests were priced with an assumption, such as an unknown cache lifetime.";
  return (
    <Tip content={why}>
      <span tabIndex={0} className="underline decoration-dotted underline-offset-2">
        {value}
      </span>
    </Tip>
  );
}

type RowColumn = Column<SessionRow>;

const LAST_ACTIVE: RowColumn = {
  header: "Last active",
  cell: (r) => <span className="whitespace-nowrap">{fmt.dateTime(r.endedAt)}</span>,
};
const SESSION: RowColumn = {
  header: "Session",
  cell: (r) => (
    <span className="font-mono text-xs">
      {fmt.shortId(r.key)}
      {r.isSubagent ? (
        <span className="ml-1.5 rounded-full border border-border px-1.5 font-sans text-[11px] text-muted">
          subagent
        </span>
      ) : null}
    </span>
  ),
};
const PROJECT: RowColumn = {
  header: "Project",
  cell: (r) =>
    r.project === null ? (
      "—"
    ) : (
      <Tip content={r.project}>
        <span tabIndex={0}>{fmt.basename(r.project)}</span>
      </Tip>
    ),
};
const MODELS: RowColumn = {
  header: "Models",
  cell: (r) => <span className="text-xs text-muted">{modelNames(r.models)}</span>,
};
const NUMBERS: readonly RowColumn[] = [
  { header: "Requests", numeric: true, cell: (r) => fmt.integer(r.measures.requests) },
  { header: "Tokens", numeric: true, cell: (r) => MEASURE.tokens.format(r.measures.tokens) },
  { header: "Cost", numeric: true, cell: (r) => <Cost row={r} /> },
  { header: "Active", numeric: true, cell: (r) => fmt.duration(r.activeMs) },
  { header: "Duration", numeric: true, cell: (r) => fmt.duration(r.endedAt - r.startedAt) },
];

/** The explorer: every column. */
export const SESSION_COLUMNS: readonly RowColumn[] = [
  LAST_ACTIVE,
  SESSION,
  PROJECT,
  MODELS,
  ...NUMBERS,
];

/** Subagents of one session share its project, so it is not repeated. */
export const SUBAGENT_COLUMNS: readonly RowColumn[] = [SESSION, MODELS, ...NUMBERS];
