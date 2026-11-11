/** Forms for billing entries and budgets, with the core's errors inline. */
import type {
  BillingEntryInput,
  BillingKind,
  BillingRecurrence,
  BudgetInput,
  ProjectInfo,
} from "@veyra/contracts";
import { useState, type ReactNode } from "react";

import { useCommandMutation } from "@/shared/api";
import { FormActions, FormError, Modal, SelectField, TextField } from "@/shared/ui";

import { KIND_LABEL, RECURRENCE_LABEL, fromNano, toNano } from "./billing-model";

const options = <T extends string>(labels: Readonly<Record<T, string>>) =>
  (Object.keys(labels) as T[]).map((value) => ({ value, label: labels[value] }));
const KINDS = options<BillingKind>(KIND_LABEL);
const RECURRENCES = options<BillingRecurrence>(RECURRENCE_LABEL);

type Amount = ReturnType<typeof useAmount>;

/** An amount typed as text, read exactly; invalid text submits as zero (refused). */
function useAmount(nano: number) {
  const [text, setText] = useState(nano === 0 ? "" : fromNano(nano));
  const value = toNano(text);
  return { text, setText, value, invalid: text !== "" && value === null };
}

/** A dialog holding one form: its fields, an explanation, the error and the actions. */
function FormDialog({
  title,
  submit = "Save",
  note,
  error,
  busy,
  onSubmit,
  onClose,
  children,
}: {
  title: string;
  submit?: string;
  note: string;
  error: Error | null;
  busy: boolean;
  onSubmit: () => void;
  onClose: () => void;
  children: ReactNode;
}) {
  return (
    <Modal
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title={title}
      showTitle
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          onSubmit();
        }}
        className="grid gap-3 p-5"
      >
        {children}
        <p className="text-xs text-muted">{note}</p>
        <FormError error={error} />
        <FormActions busy={busy} submit={submit} onCancel={onClose} />
      </form>
    </Modal>
  );
}

function ScheduleFields({
  entry,
  set,
}: {
  entry: BillingEntryInput;
  set: (patch: Partial<BillingEntryInput>) => void;
}) {
  const once = entry.recurrence === "once";
  return (
    <>
      <TextField
        label={once ? "Date" : "Starts on"}
        type="date"
        value={entry.startsOn}
        onChange={(startsOn) => {
          set({ startsOn });
        }}
      />
      {once ? null : (
        <TextField
          label="Ends on (optional)"
          type="date"
          value={entry.endsOn ?? ""}
          onChange={(endsOn) => {
            set({ endsOn: endsOn === "" ? null : endsOn });
          }}
        />
      )}
    </>
  );
}

function EntryFields({
  entry,
  set,
  amount,
}: {
  entry: BillingEntryInput;
  set: (patch: Partial<BillingEntryInput>) => void;
  amount: Amount;
}) {
  return (
    <div className="grid grid-cols-2 gap-3">
      <SelectField
        label="Kind"
        value={entry.kind}
        options={KINDS}
        onChange={(kind) => {
          set({ kind });
        }}
      />
      <SelectField
        label="Repeats"
        value={entry.recurrence}
        options={RECURRENCES}
        onChange={(recurrence) => {
          set(recurrence === "once" ? { recurrence, endsOn: null } : { recurrence });
        }}
      />
      <TextField
        label="Amount"
        inputMode="decimal"
        value={amount.text}
        aria-invalid={amount.invalid}
        placeholder="20.00"
        onChange={amount.setText}
      />
      <TextField
        label="Currency"
        value={entry.currency}
        maxLength={3}
        onChange={(currency) => {
          set({ currency: currency.toUpperCase() });
        }}
      />
      <ScheduleFields entry={entry} set={set} />
    </div>
  );
}

export function EntryDialog({
  id,
  initial,
  onClose,
}: {
  id: number | null;
  initial: BillingEntryInput;
  onClose: () => void;
}) {
  const save = useCommandMutation("save_billing_entry");
  const [entry, setEntry] = useState(initial);
  const amount = useAmount(initial.amountNano);
  const set = (patch: Partial<BillingEntryInput>) => {
    setEntry((e) => ({ ...e, ...patch }));
  };
  return (
    <FormDialog
      title={id === null ? "Add billing entry" : "Edit billing entry"}
      note="Subscriptions count as subscription spend and charges as actual charges. Neither is ever added to the API-equivalent estimate. Only US dollar entries are totalled."
      error={save.error}
      busy={save.isPending}
      onSubmit={() => {
        save.mutate(
          { id, entry: { ...entry, amountNano: amount.value ?? 0 } },
          { onSuccess: onClose },
        );
      }}
      onClose={onClose}
    >
      <TextField
        label="Name"
        value={entry.label}
        maxLength={80}
        placeholder="e.g. Team plan"
        onChange={(label) => {
          set({ label });
        }}
      />
      <EntryFields entry={entry} set={set} amount={amount} />
    </FormDialog>
  );
}

function BudgetFields({
  budget,
  set,
  limit,
}: {
  budget: BudgetInput;
  set: (patch: Partial<BudgetInput>) => void;
  limit: Amount;
}) {
  return (
    <div className="grid grid-cols-2 gap-3">
      <TextField
        label="Monthly limit (USD)"
        inputMode="decimal"
        value={limit.text}
        aria-invalid={limit.invalid}
        placeholder="100.00"
        onChange={limit.setText}
      />
      <TextField
        label="Warn at (% of limit)"
        type="number"
        min={1}
        max={100}
        value={String(budget.warnPercent)}
        onChange={(value) => {
          set({ warnPercent: Number(value) });
        }}
      />
    </div>
  );
}

export function BudgetDialog({
  id,
  initial,
  projects,
  onClose,
}: {
  id: number | null;
  initial: BudgetInput;
  projects: readonly ProjectInfo[];
  onClose: () => void;
}) {
  const save = useCommandMutation("save_budget");
  const [budget, setBudget] = useState(initial);
  const limit = useAmount(initial.limitNanoUsd);
  const set = (patch: Partial<BudgetInput>) => {
    setBudget((b) => ({ ...b, ...patch }));
  };
  const covers = [
    { value: "", label: "All usage" },
    ...projects.map((p) => ({ value: String(p.id), label: p.label })),
  ];
  return (
    <FormDialog
      title={id === null ? "Add budget" : "Edit budget"}
      note="A budget is a monthly limit on API-equivalent cost, an estimate rather than a bill."
      error={save.error}
      busy={save.isPending}
      onSubmit={() => {
        save.mutate(
          { id, budget: { ...budget, limitNanoUsd: limit.value ?? 0 } },
          { onSuccess: onClose },
        );
      }}
      onClose={onClose}
    >
      <TextField
        label="Name"
        value={budget.label}
        maxLength={80}
        placeholder="e.g. Monthly AI spend"
        onChange={(label) => {
          set({ label });
        }}
      />
      <BudgetFields budget={budget} set={set} limit={limit} />
      <SelectField
        label="Covers"
        value={budget.projectId === null ? "" : String(budget.projectId)}
        options={covers}
        onChange={(value) => {
          set({ projectId: value === "" ? null : Number(value) });
        }}
      />
    </FormDialog>
  );
}

/** Confirms deleting a record the user created; nothing recorded by a source changes. */
export function DeleteDialog({
  what,
  busy,
  error,
  onConfirm,
  onClose,
}: {
  what: string;
  busy: boolean;
  error: Error | null;
  onConfirm: () => void;
  onClose: () => void;
}) {
  return (
    <FormDialog
      title={`Delete ${what}?`}
      submit="Delete"
      note="This removes it permanently. Usage recorded by your tools is not affected."
      error={error}
      busy={busy}
      onSubmit={onConfirm}
      onClose={onClose}
    >
      {null}
    </FormDialog>
  );
}
