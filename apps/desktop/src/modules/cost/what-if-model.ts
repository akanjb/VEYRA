/** Presentation helpers for the what-if card; every figure comes from the core. */
import type { WhatIfChoice, WhatIfComparison } from "@veyra/contracts";

import { MEASURE, fmt } from "@/shared/format";

/** Choices grouped by provider, in the core's order. */
export function choiceGroups(
  choices: readonly WhatIfChoice[],
): { provider: string; choices: WhatIfChoice[] }[] {
  const groups: { provider: string; choices: WhatIfChoice[] }[] = [];
  for (const choice of choices) {
    const last = groups.at(-1);
    if (last?.provider === choice.provider.label) last.choices.push(choice);
    else groups.push({ provider: choice.provider.label, choices: [choice] });
  }
  return groups;
}

/** "18% less" or "40% more": a relative change in words; `null` when unknown. */
export function changeText(change: number | null): string | null {
  if (change === null) return null;
  if (change === 0) return "the same";
  const size = fmt.percent(Math.abs(change));
  return change < 0 ? `${size} less` : `${size} more`;
}

/** The headline: what the compared tokens cost, and would cost on the target. */
export function summaryText(comparison: WhatIfComparison): string {
  const cost = MEASURE.cost.exact;
  const change = changeText(comparison.change);
  const repriced = `${cost(comparison.repricedNanoUsd)} on ${comparison.target.model.label}`;
  const recorded = `${cost(comparison.currentNanoUsd)} as used`;
  return change === null ? `${repriced} · ${recorded}` : `${repriced} (${change}) · ${recorded}`;
}

/** Why some tokens are not compared, or `null` when every token is. */
export function leftOutText(comparison: WhatIfComparison): string | null {
  if (comparison.tokensLeftOut === 0) return null;
  return `${fmt.compact(comparison.tokensLeftOut)} tokens of models with no published price are left out of both sides.`;
}
