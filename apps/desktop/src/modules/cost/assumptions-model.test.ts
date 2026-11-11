import type { CostAssumptions, PricingTally, RuleUse } from "@veyra/contracts";
import { describe, expect, it } from "vitest";

import {
  coveredModels,
  effectiveText,
  modifierText,
  rateRows,
  summaryText,
} from "./assumptions-model";

const tally: PricingTally = { events: 1, tokens: 10, apiEquivalentNanoUsd: 5 };

const rule = (extra: Partial<RuleUse> = {}): RuleUse => ({
  id: "r",
  provider: { key: "acme", label: "Acme" },
  models: "acme-1",
  isPrefix: false,
  effectiveFrom: null,
  effectiveUntil: null,
  base: {
    input: 2_000_000,
    output: 10_000_000,
    cacheRead: 200_000,
    cacheWriteShort: null,
    cacheWriteLong: null,
    reasoning: null,
  },
  tiers: [],
  modifiers: [],
  sourceUrl: "https://example.com",
  verifiedAt: "2026-11-01",
  isStale: false,
  historyAssumed: false,
  tally,
  ...extra,
});

describe("cost assumptions wording", () => {
  it("lists only the rates that are published", () => {
    expect(rateRows(rule().base).map((r) => r.label)).toEqual(["Input", "Output", "Cache read"]);
    expect(rateRows(rule().base)[0]?.value).toMatch(/^\$2\.00 \/ MTok$/);
  });

  it("words modifiers with their cache-read factor", () => {
    expect(modifierText({ kind: "batch", factorBps: 5_000, cacheReadFactorBps: null })).toBe(
      "Batch 0.5×",
    );
    expect(modifierText({ kind: "priority", factorBps: 18_000, cacheReadFactorBps: 10_000 })).toBe(
      "Priority 1.8× (cache reads 1×)",
    );
  });

  it("names covered models and effective dates", () => {
    expect(coveredModels(rule())).toBe("acme-1");
    expect(coveredModels(rule({ models: "acme-", isPrefix: true }))).toContain("starting with");
    expect(effectiveText(rule())).toBeNull();
    expect(effectiveText(rule({ effectiveFrom: "2026-01-01" }))).toMatch(/^From /);
    expect(effectiveText(rule({ effectiveUntil: "2026-01-01" }))).toMatch(/^Until /);
    expect(
      effectiveText(rule({ effectiveFrom: "2026-01-01", effectiveUntil: "2026-02-01" })),
    ).toContain("–");
  });

  it("summarises applied, stale and unpriced", () => {
    const data: CostAssumptions = {
      catalogVersion: "v",
      staleAfterDays: 120,
      statuses: [],
      assumptions: [],
      rules: [rule(), rule({ id: "s", isStale: true })],
      unpriced: [{ provider: { key: "acme", label: "Acme" }, model: null, tally }],
      definition: "",
    };
    expect(summaryText(data)).toBe("2 prices applied · 1 stale · 1 model without a price");
    expect(summaryText({ ...data, rules: [rule()], unpriced: [] })).toBe("1 price applied");
  });
});
