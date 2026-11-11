import type { ProjectInfo, ProjectRow } from "@veyra/contracts";
import { useId, useState } from "react";

import { useCommand } from "@/shared/api";
import { DataTable } from "@/shared/charts";
import { useFilters } from "@/shared/filters";
import { MEASURE, MISSING, fmt } from "@/shared/format";
import { useOpenProject } from "@/shared/links";
import { Button, Card, EmptyState, ErrorState, PageSkeleton, Sparkline } from "@/shared/ui";

import { MergeDialog, RenameDialog } from "./project-dialogs";
import { folderOf, searchProjects } from "./projects-model";

type Editing = { kind: "rename" | "merge"; project: ProjectInfo } | null;

function ProjectCell({ project }: { project: ProjectInfo }) {
  const folder = folderOf(project);
  return (
    <div className="grid min-w-0">
      <span className="font-medium break-all">{project.label}</span>
      {folder === null ? null : <span className="text-xs break-all text-muted">{folder}</span>}
      {project.aliases.length === 0 ? null : (
        <span className="text-xs text-muted">
          + {project.aliases.length} merged {project.aliases.length === 1 ? "folder" : "folders"}
        </span>
      )}
    </div>
  );
}

function RowActions({ project, edit }: { project: ProjectInfo; edit: (e: Editing) => void }) {
  // Buttons inside a clickable row: keep their clicks and keys from opening the project.
  const stop = (event: { stopPropagation: () => void }) => {
    event.stopPropagation();
  };
  return (
    <div className="flex justify-end gap-1" onClick={stop} onKeyDown={stop} role="group">
      <Button
        size="sm"
        variant="ghost"
        aria-label={`Rename ${project.label}`}
        onClick={() => {
          edit({ kind: "rename", project });
        }}
      >
        Rename
      </Button>
      <Button
        size="sm"
        variant="ghost"
        aria-label={`Merge ${project.label} into another project`}
        onClick={() => {
          edit({ kind: "merge", project });
        }}
      >
        Merge
      </Button>
    </div>
  );
}

function ProjectTable({ rows, edit }: { rows: readonly ProjectRow[]; edit: (e: Editing) => void }) {
  const open = useOpenProject();
  return (
    <DataTable<ProjectRow>
      caption="Projects"
      rows={rows}
      rowKey={(r) => String(r.project.id)}
      onRowClick={(r) => {
        open(r.project.id);
      }}
      columns={[
        { header: "Project", cell: (r) => <ProjectCell project={r.project} /> },
        {
          header: "Daily tokens",
          cell: (r) => <Sparkline values={r.dailyTokens} className="w-28" />,
        },
        { header: "Tokens", numeric: true, cell: (r) => MEASURE.tokens.format(r.measures.tokens) },
        { header: "Cost", numeric: true, cell: (r) => MEASURE.cost.format(r.measures.costNanoUsd) },
        { header: "Sessions", numeric: true, cell: (r) => fmt.integer(r.sessions) },
        { header: "Active time", numeric: true, cell: (r) => fmt.duration(r.activeMs) },
        { header: "Top model", cell: (r) => r.topModel?.label ?? MISSING },
        {
          header: "Last active",
          cell: (r) => (r.lastActiveAt === null ? MISSING : fmt.ago(r.lastActiveAt)),
        },
        { header: "Actions", cell: (r) => <RowActions project={r.project} edit={edit} /> },
      ]}
    />
  );
}

/** Projects: which ones drive usage, with renaming and merging (03-ux.md §4, Projects). */
export function ProjectsPage() {
  const filters = useFilters();
  const projects = useCommand("list_projects", { scope: filters.scope });
  const [search, setSearch] = useState("");
  const [editing, setEditing] = useState<Editing>(null);
  const field = useId();
  if (projects.isPending) return <PageSkeleton label="Loading projects" />;
  if (projects.isError)
    return <ErrorState error={projects.error} onRetry={() => void projects.refetch()} />;

  const all = projects.data.rows;
  const shown = searchProjects(all, search);
  const close = () => {
    setEditing(null);
  };
  return (
    <Card
      title="Projects"
      definition="Every project (working directory) with its usage in the period, the busiest first. Rename a project, or merge projects that are really one: merging moves usage, never deletes it, and can be undone."
      actions={
        <label htmlFor={field} className="flex items-center gap-2 text-xs text-muted">
          Search
          <input
            id={field}
            type="search"
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
            }}
            className="h-7 w-48 rounded-control border border-border bg-surface px-2 text-sm text-text"
          />
        </label>
      }
    >
      {all.length === 0 ? (
        <EmptyState title="No projects yet">
          Projects appear once a source records usage in a working directory.
        </EmptyState>
      ) : (
        <div className="overflow-x-auto">
          <ProjectTable rows={shown} edit={setEditing} />
        </div>
      )}
      {editing?.kind === "rename" ? (
        <RenameDialog project={editing.project} onClose={close} />
      ) : null}
      {editing?.kind === "merge" ? (
        <MergeDialog
          project={editing.project}
          projects={all.map((r) => r.project)}
          onClose={close}
        />
      ) : null}
    </Card>
  );
}
