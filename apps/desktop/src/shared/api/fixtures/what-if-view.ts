/**
 * The what-if response built from the synthetic dataset, shaped like the core's: each
 * fixture model's tokens priced at another fixture model's rates. Only for development
 * and UI tests.
 */
import type { Scope, WhatIf, WhatIfChoice, WhatIfRow } from "@veyra/contracts";

import { ApiCallError } from "../api-errors";
import { select, type Flat } from "./aggregate";
import { FIXTURE_MODELS, fixtureLabel } from "./dataset";

/** Nano-USD for `tokens` at `usdPerMTok`. */
const nano = (tokens: number, usdPerMTok: number) => Math.round(tokens * usdPerMTok * 1_000);
const relative = (current: number, repriced: number) =>
  current === 0 ? null : (repriced - current) / current;

const CHOICES: WhatIfChoice[] = FIXTURE_MODELS.map(([id, provider]) => ({
  model: fixtureLabel(id),
  provider: fixtureLabel(provider),
}));

type FixtureModel = (typeof FIXTURE_MODELS)[number];

/** One fixture model's tokens and cost, and the same tokens at the target's rates. */
function row(
  selected: readonly Flat[],
  [id, provider]: FixtureModel,
  target: FixtureModel,
): WhatIfRow {
  const [, , , input, output, cacheRead, cacheWrite] = target;
  const events = selected.filter((f) => f.event.model === id).map((f) => f.event);
  const sum = (pick: (e: (typeof events)[number]) => number) =>
    events.reduce((total, e) => total + pick(e), 0);
  const current = sum((e) => e.costNanoUsd);
  const repriced =
    nano(
      sum((e) => e.input),
      input,
    ) +
    nano(
      sum((e) => e.output),
      output,
    ) +
    nano(
      sum((e) => e.cacheRead),
      cacheRead,
    ) +
    nano(
      sum((e) => e.cacheWrite),
      cacheWrite,
    );
  return {
    key: id,
    label: fixtureLabel(id).label,
    provider,
    tokens: sum((e) => e.input + e.output + e.cacheRead + e.cacheWrite),
    currentNanoUsd: current,
    repricedNanoUsd: repriced,
    change: relative(current, repriced),
  };
}

export function whatIf(flat: readonly Flat[], scope: Scope, model: string | null): WhatIf {
  const definition =
    "The tokens each model used in the period, priced at another model's current published rates at the standard service tier, beside their API-equivalent cost.";
  if (model === null) return { choices: CHOICES, comparison: null, definition };
  const target = FIXTURE_MODELS.find(([id]) => id === model);
  const choice = CHOICES.find((c) => c.model.key === model);
  if (target === undefined || choice === undefined) {
    throw new ApiCallError("get_what_if", {
      kind: "validation",
      message: `model "${model}" has no current published price`,
    });
  }
  const selected = select(flat, scope, scope.start, scope.end);
  const rows = FIXTURE_MODELS.map((m) => row(selected, m, target))
    .filter((r) => r.tokens > 0)
    .sort((a, b) => b.currentNanoUsd - a.currentNanoUsd);
  const current = rows.reduce((t, r) => t + r.currentNanoUsd, 0);
  const repriced = rows.reduce((t, r) => t + r.repricedNanoUsd, 0);
  return {
    choices: CHOICES,
    comparison: {
      target: choice,
      currentNanoUsd: current,
      repricedNanoUsd: repriced,
      change: relative(current, repriced),
      tokensCompared: rows.reduce((t, r) => t + r.tokens, 0),
      tokensLeftOut: 0,
      rows,
      assumptions: [],
    },
    definition,
  };
}
