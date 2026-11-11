/**
 * The cost assumptions response built from the synthetic dataset, shaped like the core's.
 * One price rule per fixture model; one rule's rates are older than the stale limit, and
 * requests made before a rule was checked carry the "history assumed" assumption. Only
 * for development and UI tests.
 */
import type { CostAssumptions, PricingTally, RuleUse, Scope } from "@veyra/contracts";

import { select, type Flat } from "./aggregate";
import { FIXTURE_MODELS, fixtureLabel } from "./dataset";

const DAY = 86_400_000;
const STALE_AFTER_DAYS = 120;
/** Days before "now" each fixture model's rates were checked, by model position. */
const CHECKED_DAYS_AGO = [10, 30, 200] as const;
const HISTORY =
  "The request is older than the date its price was checked; the price is assumed not to have changed since.";

const micro = (usdPerMTok: number) => Math.round(usdPerMTok * 1_000_000);
const isoDate = (at: number) => new Date(at).toISOString().slice(0, 10);

function tallyOf(flat: readonly Flat[]): PricingTally {
  return flat.reduce<PricingTally>(
    (t, { event: e }) => ({
      events: t.events + 1,
      tokens: t.tokens + e.input + e.cacheRead + e.cacheWrite + e.output,
      apiEquivalentNanoUsd: t.apiEquivalentNanoUsd + e.costNanoUsd,
    }),
    { events: 0, tokens: 0, apiEquivalentNanoUsd: 0 },
  );
}

type FixtureModel = (typeof FIXTURE_MODELS)[number];

function rule(model: FixtureModel, daysAgo: number, flat: readonly Flat[], now: number): RuleUse {
  const [id, provider, , input, output, cacheRead, cacheWrite] = model;
  return {
    id: `${id}-rule`,
    provider: fixtureLabel(provider),
    models: id,
    isPrefix: false,
    effectiveFrom: null,
    effectiveUntil: null,
    base: {
      input: micro(input),
      output: micro(output),
      cacheRead: micro(cacheRead),
      cacheWriteShort: micro(cacheWrite),
      cacheWriteLong: micro(cacheWrite * 1.6),
      reasoning: null,
    },
    tiers: [],
    modifiers: [{ kind: "batch", factorBps: 5_000, cacheReadFactorBps: null }],
    sourceUrl: "https://example.com/pricing",
    verifiedAt: isoDate(now - daysAgo * DAY),
    isStale: daysAgo > STALE_AFTER_DAYS,
    historyAssumed: true,
    tally: tallyOf(flat),
  };
}

export function costAssumptions(flat: readonly Flat[], scope: Scope, now: number): CostAssumptions {
  const selected = select(flat, scope, scope.start, scope.end);
  const rules = FIXTURE_MODELS.map((model, index) =>
    rule(
      model,
      CHECKED_DAYS_AGO[index] ?? 0,
      selected.filter((f) => f.event.model === model[0]),
      now,
    ),
  )
    .filter((r) => r.tally.events > 0)
    .sort((a, b) => b.tally.apiEquivalentNanoUsd - a.tally.apiEquivalentNanoUsd);
  const checkedAt = new Map(rules.map((r) => [r.models, Date.parse(r.verifiedAt)]));
  const history = selected.filter((f) => f.event.at < (checkedAt.get(f.event.model) ?? 0));
  const total = tallyOf(selected);
  return {
    catalogVersion: "fixture",
    staleAfterDays: STALE_AFTER_DAYS,
    statuses: total.events > 0 ? [{ status: "priced", tally: total }] : [],
    assumptions:
      history.length > 0
        ? [
            {
              kind: "history-assumed",
              description: HISTORY,
              makesPartial: false,
              tally: tallyOf(history),
            },
          ]
        : [],
    rules,
    unpriced: [],
    definition:
      "How API-equivalent cost was estimated for the period: the published prices applied, anything that had to be assumed, and models with no published price, which add nothing to the cost.",
  };
}
