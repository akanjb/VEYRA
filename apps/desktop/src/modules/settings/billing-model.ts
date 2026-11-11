/**
 * Pure helpers for editing billing entries and budgets: exact conversion between typed
 * amounts and integer billionths (ADR-004), labels and empty drafts. The core validates
 * every saved record; these only shape what the user types.
 */
import type {
  BillingEntryInput,
  BillingKind,
  BillingRecurrence,
  BudgetInput,
} from "@veyra/contracts";

const NANO_DIGITS = 9;

/**
 * A typed amount such as "20", "19.99" or "1,250.5" as integer billionths, exactly (no
 * floating point). `null` for anything that is not a non-negative decimal with at most
 * nine decimals.
 */
export function toNano(text: string): number | null {
  const cleaned = text.trim().replaceAll(",", "");
  const match = /^(\d+)(?:\.(\d{0,9}))?$/.exec(cleaned);
  if (match === null) return null;
  const whole = match[1] ?? "0";
  const fraction = (match[2] ?? "").padEnd(NANO_DIGITS, "0");
  const nano = Number(whole) * 10 ** NANO_DIGITS + Number(fraction);
  return Number.isSafeInteger(nano) ? nano : null;
}

/** Integer billionths as a plain decimal for an input, without trailing zeros. */
export function fromNano(nano: number): string {
  const whole = Math.trunc(nano / 10 ** NANO_DIGITS);
  const fraction = String(nano % 10 ** NANO_DIGITS)
    .padStart(NANO_DIGITS, "0")
    .replace(/0+$/, "");
  return fraction === "" ? String(whole) : `${String(whole)}.${fraction}`;
}

export const KIND_LABEL: Readonly<Record<BillingKind, string>> = {
  subscription: "Subscription",
  charge: "Charge",
};

export const RECURRENCE_LABEL: Readonly<Record<BillingRecurrence, string>> = {
  once: "Once",
  monthly: "Monthly",
  yearly: "Yearly",
};

/** A new entry: a monthly subscription in US dollars starting today. */
export function emptyEntry(today: string): BillingEntryInput {
  return {
    kind: "subscription",
    label: "",
    provider: null,
    amountNano: 0,
    currency: "USD",
    recurrence: "monthly",
    startsOn: today,
    endsOn: null,
  };
}

/** A new budget over all usage. */
export function emptyBudget(warnPercent: number): BudgetInput {
  return { label: "", limitNanoUsd: 0, warnPercent, projectId: null };
}

/** How an entry repeats, in words: "Monthly from 1 Nov 2026 until 31 Oct 2027". */
export function scheduleOf(entry: BillingEntryInput, day: (iso: string) => string): string {
  if (entry.recurrence === "once") return `Once on ${day(entry.startsOn)}`;
  const until = entry.endsOn === null ? "" : ` until ${day(entry.endsOn)}`;
  return `${RECURRENCE_LABEL[entry.recurrence]} from ${day(entry.startsOn)}${until}`;
}
