import type { ProjectDetail } from "@veyra/contracts";
import { Link, useParams } from "@tanstack/react-router";
import { useState } from "react";

import { useCommand, useCommandMutation } from "@/shared/api";
import { RankTable, Ranking, useChartTheme } from "@/shared/charts";
import { useFilters, withDay } from "@/shared/filters";
import { MEASURE, fmt } from "@/shared/format";
import { rankItems } from "@/shared/rankings";
import {
  ExpensiveSessionsCard,
  KpiStrip,
  ModelMixCard,
  TrendCard,
  hasUsage,
} from "@/shared/summary";
import {
  Button,
  Card,
  CardGrid,
  ChartCard,
  ErrorState,
  KpiTile,
  NoUsageCard,
  PageSkeleton,
} from "@/shared/ui";

import { RenameDialog } from "./project-dialogs";

function Header({ detail, onRename }: { detail: ProjectDetail; onRename: () => void }) {
  const unmerge = useCommandMutation("unmerge_project");
  const { project } = detail;
  return (
    <Card
      title={project.label}
      definition="A project is a working directory; merged folders count as part of it."
    >
      <div className="grid gap-2 text-sm">
        <nav aria-label="Breadcrumb" className="text-xs text-muted">
          <Link to="/projects" className="hover:text-text">
            Projects
          </Link>{" "}
          › {project.label}
        </nav>
        <p className="break-all text-muted">{project.path}</p>
        {project.aliases.length === 0 ? null : (
          <ul className="grid gap-1" aria-label="Merged folders">
            {project.aliases.map((alias) => (
              <li key={alias.id} className="flex items-center gap-2">
                <span className="break-all">{alias.path}</span>
                <Button
                  size="sm"
                  variant="ghost"
                  aria-label={`Unmerge ${alias.path}`}
                  disabled={unmerge.isPending}
                  onClick={() => {
                    unmerge.mutate({ id: alias.id });
                  }}
                >
                  Unmerge
                </Button>
              </li>
            ))}
          </ul>
        )}
        <div>
          <Button size="sm" onClick={onRename}>
            Rename
          </Button>
        </div>
      </div>
    </Card>
  );
}

function ProvidersCard({ detail }: { detail: ProjectDetail }) {
  const theme = useChartTheme();
  const items = rankItems(
    detail.providers,
    { pick: (m) => m.tokens, coloring: "category", noun: "providers" },
    theme,
  );
  return (
    <ChartCard
      title="Providers"
      definition="Tokens per provider in this project."
      accuracy={detail.composition.weakest}
      chart={<Ranking items={items} format={MEASURE.tokens.format} label="Tokens per provider" />}
      table={
        <RankTable
          items={items}
          format={MEASURE.tokens.exact}
          caption="Tokens per provider"
          header="Provider"
        />
      }
    />
  );
}

/** The project's cache benefit and hit ratio. */
function CacheTiles({ detail }: { detail: ProjectDetail }) {
  return (
    <CardGrid>
      <KpiTile
        label="Net cache benefit"
        kpi={detail.netCacheBenefitNanoUsd}
        format={MEASURE.cost.format}
        exact={MEASURE.cost.exact}
      />
      <KpiTile
        label="Cache hit ratio"
        kpi={detail.kpis.cacheHitRatio}
        format={(v) => fmt.percent(v)}
      />
    </CardGrid>
  );
}

/** One project over the period (docs/planning/03-ux.md §4, Project detail). */
export function ProjectDetailPage() {
  const params: Record<string, string | undefined> = useParams({ strict: false });
  const id = Number(params["projectId"]);
  const filters = useFilters();
  const detail = useCommand("get_project", { scope: filters.scope, id });
  const [renaming, setRenaming] = useState(false);
  if (detail.isPending) return <PageSkeleton label="Loading project" />;
  if (detail.isError)
    return <ErrorState error={detail.error} onRetry={() => void detail.refetch()} />;

  const data = detail.data;
  const { state, setState } = filters;
  return (
    <div className="grid gap-4">
      <Header
        detail={data}
        onRename={() => {
          setRenaming(true);
        }}
      />
      <KpiStrip summary={data} measure={state.measure} />
      {hasUsage(data) ? (
        <>
          <TrendCard
            summary={data}
            measure={state.measure}
            onSelectDate={(date) => {
              setState(withDay(state, date));
            }}
          />
          <CardGrid>
            <ModelMixCard summary={data} measure={state.measure} />
            <ProvidersCard detail={data} />
          </CardGrid>
          <CacheTiles detail={data} />
          <ExpensiveSessionsCard
            sessions={data.sessions}
            definition="This project's sessions with the highest API-equivalent cost in the period."
          />
        </>
      ) : (
        <NoUsageCard title="Project usage" />
      )}
      {renaming ? (
        <RenameDialog
          project={data.project}
          onClose={() => {
            setRenaming(false);
          }}
        />
      ) : null}
    </div>
  );
}
