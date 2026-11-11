import { describe, expect, it } from "vitest";

import { highlightText } from "./highlights-model";

describe("highlight wording", () => {
  it("phrases each kind from its figures", () => {
    expect(
      highlightText({ kind: "busiestDay", date: "2026-11-08", tokens: 1_200_000 }).label,
    ).toMatch(/^Busiest day: /);
    expect(
      highlightText({ kind: "topModel", label: "Opus", provider: "anthropic", share: 0.6 }),
    ).toEqual({ label: "Top model: Opus", value: "60% of tokens" });
    expect(highlightText({ kind: "topProject", key: "3", label: "veyra", share: 0.25 }).label).toBe(
      "Top project: veyra",
    );
    expect(highlightText({ kind: "cacheSaved", netNanoUsd: 4_000_000_000 }).value).toBe("≈$4.00");
    expect(highlightText({ kind: "costChange", change: -0.1 }).label).toBe("Cost down");
    expect(highlightText({ kind: "costChange", change: 0.2 }).label).toBe("Cost up");
  });
});
