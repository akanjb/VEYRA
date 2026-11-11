import type {
  BillingEntryInfo,
  BillingEntryInput,
  BillingSettings,
  BudgetInput,
} from "@veyra/contracts";
import { useState } from "react";

import { useCommand, useCommandMutation } from "@/shared/api";
import { DataTable, type Column } from "@/shared/charts";
import { todayIn, useFilters } from "@/shared/filters";
import { RECORDED_MONEY, fmt } from "@/shared/format";
import { Button, Card, ErrorState, PageSkeleton } from "@/shared/ui";

import { BudgetDialog, DeleteDialog, EntryDialog } from "./billing-dialogs";
import { KIND_LABEL, emptyBudget, emptyEntry, fromNano, scheduleOf } from "./billing-model";

type Editing =
  | { kind: "entry"; id: number | null; initial: BillingEntryInput }
  | { kind: "budget"; id: number | null; initial: BudgetInput }
  | { kind: "delete-entry" | "delete-budget"; id: number; what: string }
  | null;

function RowButtons({
  label,
  onEdit,
  onDelete,
}: {
  label: string;
  onEdit: () => void;
  onDelete: () => void;
}) {
  return (
    <div className="flex justify-end gap-1">
      <Button size="sm" variant="ghost" aria-label={`Edit ${label}`} onClick={onEdit}>
        Edit
      </Button>
      <Button size="sm" variant="ghost" aria-label={`Delete ${label}`} onClick={onDelete}>
        Delete
      </Button>
    </div>
  );
}

/** The billing-entry table's columns, with edit and delete per row. */
function entryColumns(edit: (e: Editing) => void): Column<BillingEntryInfo>[] {
  return [
    { header: "Name", cell: (r) => r.entry.label },
    { header: "Kind", cell: (r) => KIND_LABEL[r.entry.kind] },
    {
      header: "Amount",
      numeric: true,
      cell: (r) =>
        r.entry.currency === "USD"
          ? RECORDED_MONEY.exact(r.entry.amountNano)
          : `${fromNano(r.entry.amountNano)} ${r.entry.currency}`,
    },
    { header: "Schedule", cell: (r) => scheduleOf(r.entry, (d) => fmt.day(d)) },
    {
      header: "Actions",
      cell: (r) => (
        <RowButtons
          label={r.entry.label}
          onEdit={() => {
            edit({ kind: "entry", id: r.id, initial: r.entry });
          }}
          onDelete={() => {
            edit({ kind: "delete-entry", id: r.id, what: r.entry.label });
          }}
        />
      ),
    },
  ];
}

function EntriesCard({
  settings,
  edit,
}: {
  settings: BillingSettings;
  edit: (e: Editing) => void;
}) {
  const today = todayIn(useFilters().scope.timeZone);
  return (
    <Card
      title="Billing entries"
      definition="Subscriptions and charges you pay, recorded by you. The Cost page shows them beside the API-equivalent estimate, never added to it."
      actions={
        <Button
          size="sm"
          onClick={() => {
            edit({ kind: "entry", id: null, initial: emptyEntry(today) });
          }}
        >
          Add entry
        </Button>
      }
    >
      {settings.entries.length === 0 ? (
        <p className="text-sm text-muted">No subscriptions or charges recorded yet.</p>
      ) : (
        <DataTable
          caption="Billing entries"
          rows={settings.entries}
          rowKey={(r) => String(r.id)}
          columns={entryColumns(edit)}
        />
      )}
    </Card>
  );
}

function BudgetsCard({
  settings,
  edit,
}: {
  settings: BillingSettings;
  edit: (e: Editing) => void;
}) {
  return (
    <Card
      title="Budgets"
      definition="Monthly limits on API-equivalent cost, for all usage or one project. The Cost page shows each against month-to-date cost and the forecast."
      actions={
        <Button
          size="sm"
          onClick={() => {
            edit({ kind: "budget", id: null, initial: emptyBudget(settings.defaultWarnPercent) });
          }}
        >
          Add budget
        </Button>
      }
    >
      {settings.budgets.length === 0 ? (
        <p className="text-sm text-muted">No budgets yet.</p>
      ) : (
        <DataTable
          caption="Budgets"
          rows={settings.budgets}
          rowKey={(r) => String(r.id)}
          columns={[
            { header: "Name", cell: (r) => r.budget.label },
            {
              header: "Monthly limit",
              numeric: true,
              cell: (r) => RECORDED_MONEY.exact(r.budget.limitNanoUsd),
            },
            { header: "Warn at", numeric: true, cell: (r) => `${String(r.budget.warnPercent)}%` },
            {
              header: "Actions",
              cell: (r) => (
                <RowButtons
                  label={r.budget.label}
                  onEdit={() => {
                    edit({ kind: "budget", id: r.id, initial: r.budget });
                  }}
                  onDelete={() => {
                    edit({ kind: "delete-budget", id: r.id, what: r.budget.label });
                  }}
                />
              ),
            },
          ]}
        />
      )}
    </Card>
  );
}

function Dialogs({ editing, onClose }: { editing: Editing; onClose: () => void }) {
  const filters = useFilters();
  const projects = useCommand(
    "list_projects",
    { scope: filters.scope },
    { enabled: editing?.kind === "budget" },
  );
  const deleteEntry = useCommandMutation("delete_billing_entry");
  const deleteBudget = useCommandMutation("delete_budget");
  if (editing === null) return null;
  switch (editing.kind) {
    case "entry":
      return <EntryDialog id={editing.id} initial={editing.initial} onClose={onClose} />;
    case "budget":
      return (
        <BudgetDialog
          id={editing.id}
          initial={editing.initial}
          projects={projects.data?.rows.map((r) => r.project) ?? []}
          onClose={onClose}
        />
      );
    case "delete-entry":
    case "delete-budget": {
      const mutation = editing.kind === "delete-entry" ? deleteEntry : deleteBudget;
      return (
        <DeleteDialog
          what={editing.what}
          busy={mutation.isPending}
          error={mutation.error}
          onConfirm={() => {
            mutation.mutate({ id: editing.id }, { onSuccess: onClose });
          }}
          onClose={onClose}
        />
      );
    }
  }
}

/** Settings: billing entries and budgets for now (docs/planning/03-ux.md §4, Settings). */
export function SettingsPage() {
  const settings = useCommand("get_billing", {});
  const [editing, setEditing] = useState<Editing>(null);
  if (settings.isPending) return <PageSkeleton label="Loading settings" />;
  if (settings.isError)
    return <ErrorState error={settings.error} onRetry={() => void settings.refetch()} />;
  return (
    <div className="grid gap-4">
      <EntriesCard settings={settings.data} edit={setEditing} />
      <BudgetsCard settings={settings.data} edit={setEditing} />
      <Dialogs
        editing={editing}
        onClose={() => {
          setEditing(null);
        }}
      />
    </div>
  );
}
