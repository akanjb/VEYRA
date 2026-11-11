/**
 * Wording of the cost assumptions panel. Every figure comes from the core; this module
 * only names and formats it.
 */
import type {
  CostAssumptions,
  ModifierKind,
  PriceModifier,
  PricingStatus,
  Rates,
  RuleUse,
} from "@veyra/contracts";

import { fmt } from "@/shared/format";

export const STATUS_LABEL: Readonly<Record<PricingStatus, string>> = {
  priced: "Priced",
  partial: "Partly priced",
  "unpriced-model": "No published price",
  "no-usage": "No tokens",
};

const MODIFIER_LABEL: Readonly<Record<ModifierKind, string>> = {
  batch: "Batch",
  flex: "Flex",
  priority: "Priority",
  fast: "Fast",
  ultrafast: "Ultrafast",
  "inference-geo-us": "US-only processing",
};

const RATE_LABEL: Readonly<Record<keyof Rates, string>> = {
  input: "Input",
  output: "Output",
  cacheRead: "Cache read",
  cacheWriteShort: "Cache write (5 min)",
  cacheWriteLong: "Cache write (1 hour)",
  reasoning: "Reasoning",
};

export interface RateRow {
  label: string;
  value: string;
}

/** The published rates of a card, leaving out kinds with no rate. */
export function rateRows(rates: Rates): RateRow[] {
  return (Object.keys(RATE_LABEL) as (keyof Rates)[]).flatMap((kind) => {
    const rate = rates[kind];
    return rate === null ? [] : [{ label: RATE_LABEL[kind], value: fmt.pricePerMTok(rate) }];
  });
}

/** "Batch 0.5× (cache reads 0.25×)". */
export function modifierText(modifier: PriceModifier): string {
  const reads =
    modifier.cacheReadFactorBps === null
      ? ""
      : ` (cache reads ${fmt.multiplier(modifier.cacheReadFactorBps)})`;
  return `${MODIFIER_LABEL[modifier.kind]} ${fmt.multiplier(modifier.factorBps)}${reads}`;
}

/** Which models a rule covers: one model, or every model whose id starts with a prefix. */
export function coveredModels(rule: RuleUse): string {
  return rule.isPrefix ? `Models starting with “${rule.models}”` : rule.models;
}

/** When a rule is in effect, or `null` when it always is. */
export function effectiveText(rule: RuleUse): string | null {
  const { effectiveFrom: from, effectiveUntil: until } = rule;
  if (from === null && until === null) return null;
  if (from === null) return `Until ${fmt.day(until ?? "")}`;
  if (until === null) return `From ${fmt.day(from)}`;
  return `${fmt.day(from)} – ${fmt.day(until)}`;
}

/** A short summary for the panel's opening line. */
export function summaryText(data: CostAssumptions): string {
  const stale = data.rules.filter((r) => r.isStale).length;
  const parts = [
    `${String(data.rules.length)} ${data.rules.length === 1 ? "price" : "prices"} applied`,
    stale > 0 ? `${String(stale)} stale` : null,
    data.unpriced.length > 0
      ? `${String(data.unpriced.length)} ${data.unpriced.length === 1 ? "model" : "models"} without a price`
      : null,
  ];
  return parts.filter((p) => p !== null).join(" · ");
}
