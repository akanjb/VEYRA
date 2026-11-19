import type { WhatIfChoice, WhatIfComparison } from "@veyra/contracts";
import { describe, expect, it } from "vitest";

import { changeText, choiceGroups, leftOutText, summaryText } from "./what-if-model";

const choice = (model: string, provider: string): WhatIfChoice => ({
  model: { key: model, label: model },
  provider: { key: provider, label: provider },
});

const comparison = (extra: Partial<WhatIfComparison> = {}): WhatIfComparison => ({
  target: choice("Small", "Acme"),
  currentNanoUsd: 2_000_000_000,
  repricedNanoUsd: 500_000_000,
  change: -0.75,
  tokensCompared: 1_000,
  tokensLeftOut: 0,
  rows: [],
  assumptions: [],
  ...extra,
});

describe("what-if model", () => {
  it("groups choices by provider in order", () => {
    const groups = choiceGroups([choice("a", "P"), choice("b", "P"), choice("c", "Q")]);
    expect(groups.map((g) => [g.provider, g.choices.length])).toEqual([
      ["P", 2],
      ["Q", 1],
    ]);
  });

  it("words a change as less or more", () => {
    expect(changeText(-0.75)).toBe("75% less");
    expect(changeText(0.5)).toBe("50% more");
    expect(changeText(0)).toBe("the same");
    expect(changeText(null)).toBeNull();
  });

  it("names the target and both costs", () => {
    const text = summaryText(comparison());
    expect(text).toContain("on Small (75% less)");
    expect(text).toContain("as used");
    expect(summaryText(comparison({ change: null }))).not.toContain("(");
  });

  it("explains tokens left out only when there are some", () => {
    expect(leftOutText(comparison())).toBeNull();
    expect(leftOutText(comparison({ tokensLeftOut: 2_000 }))).toContain("left out");
  });
});
