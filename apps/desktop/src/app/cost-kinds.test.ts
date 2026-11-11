/**
 * AC-5: no contract and no dashboard code adds the three cost kinds together (ADR-004).
 *
 * - Every money field of every generated contract is classified by its name. Actual
 *   charges and subscription spend may appear only in the reviewed fields below, each on
 *   its own, and no field name may combine kinds.
 * - No dashboard source line does arithmetic with a recorded cost kind and API-equivalent
 *   cost together.
 */
import { describe, expect, it } from "vitest";

const contracts = import.meta.glob<string>("../../../../packages/contracts/src/generated/*.ts", {
  query: "?raw",
  import: "default",
  eager: true,
});

const sources = import.meta.glob<string>(["../**/*.{ts,tsx}", "!../**/*.test.ts"], {
  query: "?raw",
  import: "default",
  eager: true,
});

/** Money fields hold integer billionths: `…NanoUsd`, or `amountNano` in a currency. */
const MONEY_FIELD = /(\w+(?:NanoUsd|Nano))\s*:/g;

/** The words that name a recorded cost kind. */
const RECORDED = /actualCharge|subscriptionSpend/;

/**
 * Where recorded cost kinds may appear: each in its own field, beside the estimate and
 * never summed with it. `amountNano` is a billing entry's amount, whose `kind` field
 * says which recorded kind it is.
 */
const REVIEWED_RECORDED_FIELDS = [
  "BillingEntryInput.amountNano",
  "CostKpis.actualChargeNanoUsd",
  "CostKpis.subscriptionSpendNanoUsd",
];

function moneyFields(): string[] {
  const fields: string[] = [];
  for (const [path, text] of Object.entries(contracts)) {
    const type = /export type (\w+)/.exec(text)?.[1];
    if (type === undefined) continue;
    for (const match of text.matchAll(MONEY_FIELD)) fields.push(`${type}.${match[1] ?? ""}`);
    expect(path).toMatch(/\.ts$/);
  }
  return fields;
}

describe("cost kinds in the contracts", () => {
  it("are read from every generated contract", () => {
    expect(Object.keys(contracts).length).toBeGreaterThan(50);
    expect(moneyFields()).toContain("Measures.costNanoUsd");
  });

  it("keep recorded kinds only in their reviewed, separate fields", () => {
    const recorded = moneyFields().filter((f) => RECORDED.test(f) || f.endsWith(".amountNano"));
    expect(recorded.sort()).toEqual([...REVIEWED_RECORDED_FIELDS].sort());
  });

  it("never name a field after more than one kind or a sum of kinds", () => {
    for (const field of moneyFields()) {
      const name = field.split(".")[1] ?? "";
      const kinds = ["actualCharge", "subscriptionSpend", "apiEquivalent"].filter((k) =>
        name.includes(k),
      );
      expect(kinds.length, field).toBeLessThanOrEqual(1);
      expect(name, field).not.toMatch(/^(total|combined|all|grand)/i);
    }
  });
});

/** An identifier of a recorded kind, and one of API-equivalent cost. */
const RECORDED_VALUE = /actualCharge\w*|subscriptionSpend\w*|\.subscription\b|\.charges?\b/;
const ESTIMATE_VALUE = /apiEquivalent\w*|costNanoUsd|\.apiEquivalent\b/;
/** Binary arithmetic: `+` or `-` between operands (not `=>`, `++`, unary signs or JSX). */
const ARITHMETIC = /[\w)\]]\s*[+-]\s*[\w(]/;

describe("cost kinds in the dashboard", () => {
  it("are read from every source file", () => {
    expect(Object.keys(sources).some((p) => p.includes("modules/cost/cost-model.ts"))).toBe(true);
  });

  it("are never added to or subtracted from each other", () => {
    const offending: string[] = [];
    for (const [path, text] of Object.entries(sources)) {
      text.split("\n").forEach((line, i) => {
        if (RECORDED_VALUE.test(line) && ESTIMATE_VALUE.test(line) && ARITHMETIC.test(line))
          offending.push(`${path}:${String(i + 1)}: ${line.trim()}`);
      });
    }
    expect(offending).toEqual([]);
  });
});
