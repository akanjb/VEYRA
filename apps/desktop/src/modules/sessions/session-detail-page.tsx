import type { SessionDetail, TimelineEvent } from "@veyra/contracts";
import { Link, useParams } from "@tanstack/react-router";
import { ChevronRight } from "lucide-react";

import { useCommand } from "@/shared/api";
import { DataTable, Timeline, useChartTheme, type Column } from "@/shared/charts";
import { useFilters } from "@/shared/filters";
import { MEASURE, fmt } from "@/shared/format";
import {
  AccuracyBadge,
  Card,
  ChartCard,
  EmptyState,
  ErrorState,
  Skeleton,
  Stat,
} from "@/shared/ui";

import { SUBAGENT_COLUMNS } from "./session-columns";
import { useOpenSession } from "@/shared/links";
import { modelNames, timelinePoints, toolCounts, type ToolCount } from "./session-model";

const EVENT_COLUMNS: readonly Column<TimelineEvent>[] = [
  { header: "Time", cell: (e) => fmt.time(e.at) },
  { header: "Model", cell: (e) => <span className="text-xs">{e.model?.label ?? "—"}</span> },
  { header: "Input", numeric: true, cell: (e) => fmt.integer(e.tokens.input) },
  { header: "Cache read", numeric: true, cell: (e) => fmt.integer(e.tokens.cacheRead) },
  { header: "Cache write", numeric: true, cell: (e) => fmt.integer(e.tokens.cacheWrite) },
  { header: "Output", numeric: true, cell: (e) => fmt.integer(e.tokens.output) },
  { header: "Cost", numeric: true, cell: (e) => MEASURE.cost.exact(e.costNanoUsd) },
  {
    header: "Tools",
    cell: (e) => (
      <span className="text-xs text-muted">{e.tools.map((t) => t.name).join(", ")}</span>
    ),
  },
];

const TOOL_COLUMNS: readonly Column<ToolCount>[] = [
  { header: "Tool", cell: (c) => <span className="font-mono text-xs">{c.name}</span> },
  { header: "Calls", numeric: true, cell: (c) => fmt.integer(c.calls) },
  { header: "Failed", numeric: true, cell: (c) => fmt.integer(c.errors) },
];

function Breadcrumb({ sessionKey }: { sessionKey: string }) {
  return (
    <nav aria-label="Breadcrumb" className="flex items-center gap-1 text-sm text-muted">
      <Link to="/sessions" className="hover:text-text">
        Sessions
      </Link>
      <ChevronRight size={14} aria-hidden />
      <span className="font-mono text-text">{fmt.shortId(sessionKey)}</span>
    </nav>
  );
}

function Summary({ detail }: { detail: SessionDetail }) {
  const s = detail.session;
  const active = (
    <span className="inline-flex items-center gap-1">
      {fmt.duration(s.activeMs)} <AccuracyBadge kind="derived" />
    </span>
  );
  return (
    <Card title="Session" accuracy={s.accuracy}>
      <dl className="grid grid-cols-2 gap-4 md:grid-cols-4 xl:grid-cols-8">
        <Stat label="Started" value={fmt.dateTime(s.startedAt)} />
        <Stat label="Duration" value={fmt.duration(s.endedAt - s.startedAt)} />
        <Stat label="Active time" value={active} />
        <Stat label="Requests" value={fmt.integer(s.measures.requests)} />
        <Stat label="Tokens" value={MEASURE.tokens.exact(s.measures.tokens)} />
        <Stat label="Cost" value={MEASURE.cost.exact(s.measures.costNanoUsd)} />
        <Stat label="Project" value={s.project === null ? "—" : fmt.basename(s.project)} />
        <Stat label="Models" value={modelNames(s.models)} />
      </dl>
    </Card>
  );
}

function TimelineCard({ detail }: { detail: SessionDetail }) {
  const theme = useChartTheme();
  const { timeZone } = useFilters();
  return (
    <ChartCard
      title="Timeline"
      definition="Each marker is one request, sized by tokens and coloured by model."
      accuracy={detail.session.accuracy}
      chart={
        <Timeline
          points={timelinePoints(detail, theme)}
          label="Requests over time"
          timeZone={timeZone}
        />
      }
      table={
        <DataTable
          caption="Requests"
          rows={detail.events}
          rowKey={(e, i) => `${String(e.at)}-${String(i)}`}
          columns={EVENT_COLUMNS}
        />
      }
    />
  );
}

function Tools({ events }: { events: readonly TimelineEvent[] }) {
  const counts = toolCounts(events);
  return (
    <Card
      title="Tool calls"
      definition="Tool names and outcomes only; Veyra never reads tool inputs or outputs."
    >
      {counts.length === 0 ? (
        <EmptyState title="No tool calls" />
      ) : (
        <DataTable
          caption="Tool calls by name"
          rows={counts}
          rowKey={(c) => c.name}
          columns={TOOL_COLUMNS}
        />
      )}
    </Card>
  );
}

function Subagents({ detail }: { detail: SessionDetail }) {
  const openSession = useOpenSession();
  return (
    <Card title="Subagents">
      {detail.subagents.length === 0 ? (
        <EmptyState title="No subagents" />
      ) : (
        <DataTable
          caption="Subagents of this session"
          rows={detail.subagents}
          rowKey={(r) => String(r.id)}
          columns={SUBAGENT_COLUMNS}
          onRowClick={(r) => {
            openSession(r.id);
          }}
        />
      )}
    </Card>
  );
}

/** One session: totals, the request timeline, tools and subagents (no content, ever). */
export function SessionDetailPage() {
  const params: Record<string, string | undefined> = useParams({ strict: false });
  const id = Number(params["sessionId"]);
  const detail = useCommand("get_session", { id }, { enabled: Number.isInteger(id) });
  if (detail.isPending) return <Skeleton className="h-96" />;
  if (detail.isError)
    return <ErrorState error={detail.error} onRetry={() => void detail.refetch()} />;
  return (
    <div className="grid gap-4">
      <Breadcrumb sessionKey={detail.data.session.key} />
      <Summary detail={detail.data} />
      <TimelineCard detail={detail.data} />
      <div className="grid gap-4 xl:grid-cols-2">
        <Tools events={detail.data.events} />
        <Subagents detail={detail.data} />
      </div>
    </div>
  );
}
