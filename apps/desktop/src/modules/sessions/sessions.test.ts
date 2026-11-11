import type { SessionDetail, TimelineEvent } from "@veyra/contracts";
import { describe, expect, it } from "vitest";

import { TEST_THEME as theme } from "@/shared/charts/test-theme";

import { timelinePoints, toolCounts } from "./session-model";
import { validateSessionsSearch } from "./sessions-search";

const event = (model: string | null, tools: TimelineEvent["tools"] = []): TimelineEvent => ({
  at: 1_000,
  provider: model === null ? null : "anthropic",
  model: model === null ? null : { key: model, label: `Name of ${model}` },
  tokens: { input: 1, cacheRead: 2, cacheWrite: 3, output: 4, reasoning: null, total: 10 },
  costNanoUsd: null,
  accuracy: "exact",
  tools,
});

describe("sessions", () => {
  it("validates the explorer URL state", () => {
    expect(validateSessionsSearch({ sort: "cost", page: "3" })).toEqual({ sort: "cost", page: 3 });
    expect(validateSessionsSearch({ sort: "lastActive", page: "0" })).toEqual({});
    expect(validateSessionsSearch({ sort: "drop table", page: "-1" })).toEqual({});
  });

  it("colours timeline points by model within the provider hue", () => {
    const detail = {
      events: [event("claude-opus-5-5"), event("claude-haiku-5-5"), event(null)],
    } as SessionDetail;
    const points = timelinePoints(detail, theme);
    expect(points.map((p) => p.group)).toEqual([
      "Name of claude-opus-5-5",
      "Name of claude-haiku-5-5",
      "Unknown model",
    ]);
    expect(points[0]?.color).toBe("#d0602a");
    expect(points[1]?.color).not.toBe(points[0]?.color);
    expect(points[0]?.detail).toBe("10 tokens · unpriced");
  });

  it("counts tool calls and failures by name", () => {
    const counts = toolCounts([
      event("m", [
        { name: "Bash", mcpServer: null, isError: true },
        { name: "Read", mcpServer: null, isError: false },
      ]),
      event("m", [{ name: "Bash", mcpServer: null, isError: false }]),
    ]);
    expect(counts).toEqual([
      { name: "Bash", mcpServer: null, calls: 2, errors: 1 },
      { name: "Read", mcpServer: null, calls: 1, errors: 0 },
    ]);
  });
});
