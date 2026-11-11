/**
 * Billing entries and budgets kept in memory, shaped like the core's responses. Only for
 * development and UI tests: the core validates and spreads billing (ADR-028).
 */
import type {
  BillingEntryInfo,
  BillingEntryInput,
  BillingSettings,
  BudgetInfo,
  BudgetInput,
} from "@veyra/contracts";

import { ApiCallError } from "../api-errors";

const DEFAULT_WARN_PERCENT = 80;

function refuse(command: string, message: string, kind: "validation" | "notFound"): never {
  throw new ApiCallError(command, { kind, message });
}

function daysInMonth(day: string): number {
  const [year = 0, month = 1] = day.split("-").map(Number);
  return new Date(Date.UTC(year, month, 0)).getUTCDate();
}

function daysInYear(day: string): number {
  const year = Number(day.slice(0, 4));
  return year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0) ? 366 : 365;
}

/** An entry's amount on one local day (an approximation of the core's accrual). */
export function amountOn(entry: BillingEntryInput, day: string): number {
  if (day < entry.startsOn || (entry.endsOn !== null && day > entry.endsOn)) return 0;
  switch (entry.recurrence) {
    case "once":
      return day === entry.startsOn ? entry.amountNano : 0;
    case "monthly":
      return Math.round(entry.amountNano / daysInMonth(day));
    case "yearly":
      return Math.round(entry.amountNano / daysInYear(day));
  }
}

export class FixtureBilling {
  private nextId = 1;
  private readonly entries = new Map<number, BillingEntryInput>();
  private readonly budgetMap = new Map<number, BudgetInput>();

  settings(): BillingSettings {
    return {
      entries: [...this.entries.entries()]
        .map(([id, entry]) => ({ id, entry }))
        .sort((a, b) => a.entry.startsOn.localeCompare(b.entry.startsOn) || a.id - b.id),
      budgets: this.budgets(),
      defaultWarnPercent: DEFAULT_WARN_PERCENT,
    };
  }

  budgets(): BudgetInfo[] {
    return [...this.budgetMap.entries()]
      .map(([id, budget]) => ({ id, budget }))
      .sort((a, b) => a.budget.label.localeCompare(b.budget.label) || a.id - b.id);
  }

  /** USD entries of one kind, summed over the given local days. */
  total(kind: BillingEntryInput["kind"], days: readonly string[]): number {
    let total = 0;
    for (const entry of this.entries.values())
      if (entry.kind === kind && entry.currency === "USD")
        for (const day of days) total += amountOn(entry, day);
    return total;
  }

  otherCurrencyEntries(): number {
    return [...this.entries.values()].filter((e) => e.currency !== "USD").length;
  }

  saveEntry(id: number | null, input: BillingEntryInput): BillingEntryInfo {
    const command = "save_billing_entry";
    const label = input.label.trim();
    if (label === "" || label.length > 80)
      refuse(command, "the label must have 1 to 80 characters", "validation");
    if (input.amountNano <= 0)
      refuse(command, "the amount must be greater than zero", "validation");
    if (input.endsOn !== null && input.recurrence === "once")
      refuse(command, "a one-off entry has no end date", "validation");
    if (input.endsOn !== null && input.endsOn < input.startsOn)
      refuse(command, "the end date is before the start date", "validation");
    if (id !== null && !this.entries.has(id))
      refuse(command, `billing entry ${String(id)} does not exist`, "notFound");
    const entry = { ...input, label, currency: input.currency.trim().toUpperCase() };
    const saved = id ?? this.nextId++;
    this.entries.set(saved, entry);
    return { id: saved, entry };
  }

  deleteEntry(id: number): void {
    if (!this.entries.delete(id))
      refuse("delete_billing_entry", `billing entry ${String(id)} does not exist`, "notFound");
  }

  saveBudget(id: number | null, input: BudgetInput): BudgetInfo {
    const command = "save_budget";
    const label = input.label.trim();
    if (label === "") refuse(command, "the label must have 1 to 80 characters", "validation");
    if (input.limitNanoUsd <= 0)
      refuse(command, "the amount must be greater than zero", "validation");
    if (input.warnPercent < 1 || input.warnPercent > 100)
      refuse(command, "the warning threshold must be between 1% and 100%", "validation");
    if (id !== null && !this.budgetMap.has(id))
      refuse(command, `budget ${String(id)} does not exist`, "notFound");
    const budget = { ...input, label };
    const saved = id ?? this.nextId++;
    this.budgetMap.set(saved, budget);
    return { id: saved, budget };
  }

  deleteBudget(id: number): void {
    if (!this.budgetMap.delete(id))
      refuse("delete_budget", `budget ${String(id)} does not exist`, "notFound");
  }
}
